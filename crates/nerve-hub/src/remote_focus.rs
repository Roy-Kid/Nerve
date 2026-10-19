//! Steering the tmux on the *other* machine, over one ssh round trip.
//!
//! A job on another machine has no pane here, and the ssh pane that reaches it
//! is the machine's single viewport (`crate::ssh`) — it shows whatever the
//! remote client is attached to, which is rarely the job you picked. What tmux
//! cannot do is show a foreign server's pane locally; what it *can* do is run
//! the same `pid → tty → pane` resolution **over there** and tell that tmux to
//! select the result. So that is what this does: `Enter` focuses the ssh pane
//! here, and one `ssh <host> sh -s` makes the far side turn to the right window.
//!
//! Deliberately not tmux control mode (`tmux -CC`, what iTerm2 speaks): control
//! mode streams a whole window tree, cursor positions and resize events so a
//! terminal emulator can render remote panes itself. This surface is a TUI
//! *inside* tmux, not an emulator — it needs one imperative "select that
//! window", not a mirror of the remote UI.
//!
//! Everything here is fail-open. The remote may have no tmux, the agent may not
//! be in a pane, ssh may need a password this process must never be asked for
//! (`BatchMode=yes`): each of those is an answer, none is an error the sidebar
//! propagates.

use std::io::Write;
use std::process::{Command, Stdio};

/// How long ssh may spend getting a connection before giving up.
///
/// A live `ControlMaster` answers in tens of milliseconds; without one this is
/// the whole budget, because the alternative is a TUI blocked on a handshake.
const CONNECT_TIMEOUT_SECS: u32 = 3;

/// Exit codes the remote script uses to say what it found.
mod code {
    pub const NO_TERMINAL: i32 = 3;
    pub const NOT_IN_TMUX: i32 = 4;
    pub const REFUSED: i32 = 5;
    pub const NO_TMUX: i32 = 6;
    /// ssh's own "I could not get there".
    pub const SSH_FAILED: i32 = 255;
}

/// What the far side did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteJump {
    /// That tmux is now showing the job. `clients` is how many were attached —
    /// the view only followed when there was exactly one to follow.
    Selected { pane: String, clients: usize },
    /// The agent process is on no pane of that tmux (a bare ssh shell, say).
    NotInTmux,
    /// The process is gone there, or has no controlling terminal.
    NoTerminal,
    /// No tmux on that machine — or none on `PATH` in a non-interactive shell.
    NoTmux,
    /// tmux was there and refused the select.
    Refused,
    /// ssh could not get there: no live master and no non-interactive auth.
    Unreachable,
}

impl RemoteJump {
    /// What to flash in the status line, or nothing when the far side simply
    /// did what was asked (the window changing under you says it better).
    pub fn message(&self, host: &str) -> Option<String> {
        match self {
            Self::Selected { clients, .. } if *clients <= 1 => None,
            Self::Selected { clients, .. } => Some(format!(
                "Nerve: selected the window on {host} ({clients} clients attached — none was moved)"
            )),
            Self::NotInTmux => Some(format!("Nerve: that job is not in a tmux pane on {host}")),
            Self::NoTerminal => Some(format!("Nerve: no terminal for that job on {host}")),
            Self::NoTmux => Some(format!("Nerve: no tmux on {host}")),
            Self::Refused => Some(format!("Nerve: tmux on {host} refused the select")),
            Self::Unreachable => Some(format!("Nerve: could not reach {host} without a prompt")),
        }
    }
}

pub fn select_job(host: &str, pid: u32) -> RemoteJump {
    let mut child = match Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            &format!("ConnectTimeout={CONNECT_TIMEOUT_SECS}"),
            "--",
            host,
            // The script arrives on stdin, so the user's remote login shell
            // (fish, csh…) never has to parse it, and nothing is quoted into a
            // command line.
            "sh",
            "-s",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return RemoteJump::Unreachable,
    };

    if let Some(stdin) = child.stdin.as_mut()
        && stdin.write_all(script(pid).as_bytes()).is_err()
    {
        let _ = child.kill();
        let _ = child.wait();
        return RemoteJump::Unreachable;
    }
    // Dropping stdin is what tells `sh -s` the script is finished.
    drop(child.stdin.take());

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(25))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return RemoteJump::Unreachable;
            }
        }
    }
    match child.wait_with_output() {
        Ok(output) => outcome(
            output.status.code(),
            &String::from_utf8_lossy(&output.stdout),
        ),
        Err(_) => RemoteJump::Unreachable,
    }
}

/// The script the far side runs.
///
/// The same resolution `crate::preview` does locally — the agent's pid sits on
/// exactly one tty, and one pane has that tty — followed by the same three tmux
/// commands `Enter` runs here. `pid` is a `u32`, so the one value interpolated
/// into this text cannot be anything but digits.
///
/// The client is only moved when exactly one is attached: with several, the one
/// in *this* ssh pane cannot be told from someone else's session, and dragging a
/// stranger's view to another window would be worse than doing nothing.
fn script(pid: u32) -> String {
    format!(
        r#"pid={pid}
tty=$(ps -p $pid -o tty= 2>/dev/null | tr -d ' \t')
[ -n "$tty" ] && [ "$tty" != "?" ] || exit {no_terminal}
command -v tmux >/dev/null 2>&1 || exit {no_tmux}
pane=$(tmux list-panes -a -F '#{{pane_tty}} #{{pane_id}}' 2>/dev/null \
  | awk -v t="/dev/$tty" '$1==t{{print $2; exit}}')
[ -n "$pane" ] || exit {not_in_tmux}
tmux select-window -t "$pane" >/dev/null 2>&1 || exit {refused}
tmux select-pane -t "$pane" >/dev/null 2>&1
clients=$(tmux list-clients -F '#{{client_tty}}' 2>/dev/null)
count=$(printf '%s\n' "$clients" | grep -c .)
if [ "$count" = 1 ]; then
  session=$(tmux display-message -p -t "$pane" '#{{session_name}}' 2>/dev/null)
  [ -n "$session" ] && tmux switch-client -c "$clients" -t "$session" >/dev/null 2>&1
fi
printf '%s %s\n' "$pane" "$count"
"#,
        pid = pid,
        no_terminal = code::NO_TERMINAL,
        no_tmux = code::NO_TMUX,
        not_in_tmux = code::NOT_IN_TMUX,
        refused = code::REFUSED,
    )
}

/// Read the far side's answer. Pure, so every arm is a test rather than a trip.
fn outcome(status: Option<i32>, stdout: &str) -> RemoteJump {
    match status {
        Some(0) => match parse_selection(stdout) {
            Some((pane, clients)) => RemoteJump::Selected { pane, clients },
            // Exit 0 with nothing to say is not a selection anyone can trust.
            None => RemoteJump::Refused,
        },
        Some(code::NO_TERMINAL) => RemoteJump::NoTerminal,
        Some(code::NOT_IN_TMUX) => RemoteJump::NotInTmux,
        Some(code::REFUSED) => RemoteJump::Refused,
        Some(code::NO_TMUX) => RemoteJump::NoTmux,
        Some(code::SSH_FAILED) | None => RemoteJump::Unreachable,
        // The remote shell could not even start the script (127 and friends).
        Some(_) => RemoteJump::NoTmux,
    }
}

/// `"%25 1"` — the pane that was selected, and how many clients were attached.
fn parse_selection(stdout: &str) -> Option<(String, usize)> {
    let line = stdout.lines().last()?.trim();
    let (pane, clients) = line.split_once(char::is_whitespace)?;
    let pane = pane.trim();
    if !pane.starts_with('%') {
        return None;
    }
    Some((pane.to_string(), clients.trim().parse().ok()?))
}
