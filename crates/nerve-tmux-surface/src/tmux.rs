//! The only place this crate talks to tmux.
//!
//! Every invocation is an arg array — there is no shell anywhere in this crate,
//! the same rule host ingest follows (arg array, no shell). A segment holds
//! spaces and `#[…]` markers and must arrive as one argument, which a shell
//! line could never guarantee.
//!
//! Writing the option is not enough: tmux only repaints the status line after
//! `refresh-client -S`. Reading uses `show-options -gqv`, which prints the bare
//! value and stays quiet about options nobody set.
//!
//! tmux user options are also this surface's only persistent state, and they
//! live in tmux rather than on disk — an option dies with the
//! server it belongs to, which is exactly the intended lifetime.

use nerve_surface_core::instance::LockStore;
use std::fmt;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

/// The user option `status-right` interpolates as `#{@nerve_status}`.
pub const STATUS_OPTION: &str = "@nerve_status";

/// Sidebar layout options (aligned with tmux-agent-sidebar naming).
pub const SIDEBAR_WIDTH: &str = "@nerve_sidebar_width";
pub const SIDEBAR_POSITION: &str = "@nerve_sidebar_position";
pub const SIDEBAR_BOTTOM_HEIGHT: &str = "@nerve_sidebar_bottom_height";
pub const PANE_ROLE: &str = "@nerve_pane_role";
pub const SIDEBAR_ROLE: &str = "nerve-sidebar";

/// Window option recording an outstanding preview swap, as `<foreign> <slot>`.
///
/// The swap moves a real pane, and a sidebar that is killed rather than closed
/// never runs its own restore — so the fact has to outlive the process. A
/// window option is exactly that lifetime.
pub const PREVIEW_SWAP: &str = "@nerve_preview_swap";

/// Pane option naming the sidebar that pulled this pane into its slot.
///
/// One tmux server, one pane per job — but a sidebar per *window*. Without a
/// claim two sidebars scanning `list-panes -a` both find the same agent pane
/// and take turns yanking it out of each other's slot, and each one's record of
/// what it swapped goes stale the moment the other wins.
pub const PREVIEW_OWNER: &str = "@nerve_preview_owner";

/// What tmux(1) prints once its socket is gone.
const SERVER_GONE_PREFIX: &str = "no server running";

/// One `tmux` invocation.
pub trait TmuxRunner {
    /// Run `tmux <args>`, answering its trimmed stdout.
    fn run(&mut self, args: &[&str]) -> Result<String, TmuxError>;
}

/// Why a tmux invocation produced no answer.
#[derive(Debug)]
pub enum TmuxError {
    /// The tmux this surface painted is gone. Not a failure: the surface's
    /// work is simply over (exit 0).
    ServerGone,
    /// tmux could not be executed at all (missing binary, spawn refused).
    Unavailable(String),
    /// tmux ran and refused the command.
    Refused { status: i32, stderr: String },
}

impl TmuxError {
    /// Classify a finished invocation from its exit status and stderr.
    pub fn from_exit(status: i32, stderr: &str) -> Self {
        let stderr = stderr.trim();
        if stderr.to_ascii_lowercase().starts_with(SERVER_GONE_PREFIX) {
            return Self::ServerGone;
        }
        Self::Refused {
            status,
            stderr: stderr.to_string(),
        }
    }

    /// Whether this is the cue to stop painting and exit 0.
    pub fn is_server_gone(&self) -> bool {
        matches!(self, Self::ServerGone)
    }
}

impl fmt::Display for TmuxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ServerGone => f.write_str("no server running"),
            Self::Unavailable(reason) => write!(f, "tmux unavailable: {reason}"),
            Self::Refused { status, stderr } => {
                write!(f, "tmux refused the command (exit {status}): {stderr}")
            }
        }
    }
}

impl std::error::Error for TmuxError {}

/// Where a rendered segment goes.
pub trait SegmentWriter {
    fn write_segment(&mut self, text: &str) -> Result<(), TmuxError>;
}

/// Paints the status option, and reads and writes the surface's user options.
pub struct TmuxWriter<R: TmuxRunner> {
    runner: R,
}

impl<R: TmuxRunner> TmuxWriter<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }
}

impl<R: TmuxRunner> SegmentWriter for TmuxWriter<R> {
    /// `set -g @nerve_status <text>`, then `refresh-client -S`.
    ///
    /// An empty segment is still written: the status line has to be told to
    /// drop what it was showing. If the option could not be set there is
    /// nothing to refresh.
    fn write_segment(&mut self, text: &str) -> Result<(), TmuxError> {
        self.runner.run(&["set", "-g", STATUS_OPTION, text])?;
        self.runner.run(&["refresh-client", "-S"])?;
        Ok(())
    }
}

/// The single-surface lock lives in a tmux user option: it dies with the
/// server it belongs to, which is exactly the lock's intended lifetime, and it
/// keeps this surface's promise to write nothing to disk.
impl<R: TmuxRunner> LockStore for TmuxWriter<R> {
    type Error = TmuxError;

    /// An unset option, and one set to blanks, read the same: absent.
    fn get(&mut self, name: &str) -> Result<Option<String>, TmuxError> {
        let value = self.runner.run(&["show-options", "-gqv", name])?;
        let value = value.trim();
        Ok((!value.is_empty()).then(|| value.to_string()))
    }

    /// Setting an option repaints nothing on its own, so it does not refresh.
    fn set(&mut self, name: &str, value: &str) -> Result<(), TmuxError> {
        self.runner.run(&["set", "-g", name, value])?;
        Ok(())
    }
}

/// The runner the binary injects: `tmux` itself.
///
/// `$TMUX` is inherited from the process tmux started, so every invocation
/// lands on the server this surface belongs to without naming a socket.
pub struct TmuxCommand;

impl TmuxRunner for TmuxCommand {
    fn run(&mut self, args: &[&str]) -> Result<String, TmuxError> {
        run_tmux_args(args)
    }
}

/// Run `tmux <args>` and return trimmed stdout.
pub fn run_tmux(args: &[&str]) -> Option<String> {
    run_tmux_args(args).ok()
}

fn run_tmux_args(args: &[&str]) -> Result<String, TmuxError> {
    let mut cmd = Command::new(tmux_binary());
    if let Some(socket) = tmux_socket() {
        cmd.arg("-S").arg(socket);
    }
    let output = cmd
        .args(args)
        .output()
        .map_err(|err| TmuxError::Unavailable(err.to_string()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TmuxError::from_exit(
            output.status.code().unwrap_or(-1),
            &stderr,
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn tmux_binary() -> &'static str {
    static BIN: OnceLock<String> = OnceLock::new();
    BIN.get_or_init(|| {
        if let Ok(path) = std::env::var("NERVE_TMUX_BIN") {
            let path = path.trim();
            if !path.is_empty() {
                return path.to_string();
            }
        }
        for candidate in [
            "/opt/homebrew/bin/tmux",
            "/usr/local/bin/tmux",
            "/usr/bin/tmux",
        ] {
            if Path::new(candidate).is_file() {
                return candidate.to_string();
            }
        }
        "tmux".to_string()
    })
}

/// Socket path from `$TMUX` (`/path/to/socket,pid,flags`).
fn tmux_socket() -> Option<String> {
    let raw = std::env::var("TMUX").ok()?;
    let socket = raw.split(',').next()?.trim();
    if socket.is_empty() {
        None
    } else {
        Some(socket.to_string())
    }
}

/// Expand a tmux format string against a target.
pub fn display_message(target: &str, format: &str) -> String {
    run_tmux(&["display-message", "-p", "-t", target, format]).unwrap_or_default()
}

/// Set a pane-scoped user option.
pub fn set_pane_option(pane: &str, name: &str, value: &str) {
    let _ = run_tmux(&["set", "-p", "-t", pane, name, value]);
}

/// Forget a pane-scoped user option.
pub fn unset_pane_option(pane: &str, name: &str) {
    let _ = run_tmux(&["set", "-p", "-u", "-t", pane, name]);
}

/// Set a window-scoped user option.
pub fn set_window_option(window: &str, name: &str, value: &str) {
    let _ = run_tmux(&["set", "-w", "-t", window, name, value]);
}

/// Read a window-scoped user option; `None` when unset or blank.
pub fn window_option(window: &str, name: &str) -> Option<String> {
    let value = display_message(window, &format!("#{{{name}}}"));
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// Forget a window-scoped user option.
pub fn unset_window_option(window: &str, name: &str) {
    let _ = run_tmux(&["set", "-w", "-u", "-t", window, name]);
}

/// `list-panes -F` format pairing a pane with its [`PANE_ROLE`].
///
/// One spelling of the `|` wire format, so renaming the option cannot leave a
/// caller matching nothing.
pub fn pane_id_role_format() -> String {
    format!("#{{pane_id}}|#{{{}}}", PANE_ROLE)
}

/// Whether tmux still knows `pane`.
pub fn pane_exists(pane: &str) -> bool {
    run_tmux(&["display-message", "-p", "-t", pane, "#{pane_id}"])
        .map(|id| !id.is_empty())
        .unwrap_or(false)
}

/// First pane in `window_id` that is neither the sidebar pane nor sidebar-roled.
pub fn sibling_content_pane(window_id: &str, sidebar_pane: &str) -> Option<String> {
    let output = run_tmux(&["list-panes", "-t", window_id, "-F", &pane_id_role_format()])?;
    output.lines().find_map(|line| {
        let (pane, role) = line.split_once('|')?;
        if pane == sidebar_pane || role == SIDEBAR_ROLE {
            None
        } else {
            Some(pane.to_string())
        }
    })
}
