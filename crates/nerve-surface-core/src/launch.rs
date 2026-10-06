//! Bringing a hub up, at most once per window.
//!
//! Every arm of [`Launch`] is fail-open: none is an error the caller may turn
//! into a non-zero exit. A missing hub, a refused spawn and a throttled retry
//! all mean the same thing to tmux — the segment shows the offline placeholder
//! and the user's tmux is untouched.
//!
//! The throttle exists because a hub that will not come up must not be fired at
//! the OS on every reconnect. A hub that *is* up is never spawned at all:
//! health is consulted first, so the one this surface started a second ago is
//! recognised rather than duplicated.

use std::fmt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use time::OffsetDateTime;

use crate::hub::Hub;

/// How long a spawn attempt suppresses the next one.
pub const THROTTLE: Duration = Duration::from_secs(10);

/// The hub's health endpoint.
const HEALTH_PATH: &str = "/v1/health";

/// The one subcommand `nerve-hub` understands (`crates/nerve-hub/src/cli.rs`).
const SERVE_ARGS: [&str; 1] = ["serve"];

/// `GET http://127.0.0.1:17890/v1/health` — is a hub already serving?
pub trait HealthProbe {
    fn is_serving(&self) -> bool;
}

/// Start a detached process. **Arg array only** — there is no shell anywhere in
/// this crate.
pub trait ProcessSpawner {
    fn spawn(&mut self, program: &Path, args: &[&str]) -> Result<(), SpawnError>;
}

/// Injected wall clock, as in `crates/nerve-hub/src/clock.rs`.
pub trait Clock {
    fn now(&self) -> OffsetDateTime;
}

/// Why the OS refused to start the hub.
#[derive(Debug)]
pub struct SpawnError(String);

impl SpawnError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SpawnError {}

/// What [`HubLauncher::ensure`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Launch {
    /// A hub already answers `/v1/health`; nothing was spawned.
    AlreadyServing,
    /// A hub was started; it is not serving yet.
    Spawned,
    /// Health is down, but the last attempt is younger than [`THROTTLE`].
    Throttled,
    /// No `nerve-hub` binary anywhere.
    Missing,
    /// The binary is there and the OS refused to start it.
    Failed,
}

impl Launch {
    /// Whether the surface paints the offline placeholder rather than a live
    /// segment.
    pub fn is_offline(self) -> bool {
        !matches!(self, Self::AlreadyServing)
    }
}

/// Decides whether a hub needs starting, and starts at most one per window.
pub struct HubLauncher<S: ProcessSpawner, C: Clock, H: HealthProbe> {
    spawner: S,
    clock: C,
    health: H,
    last_attempt: Option<OffsetDateTime>,
}

impl<S: ProcessSpawner, C: Clock, H: HealthProbe> HubLauncher<S, C, H> {
    pub fn new(spawner: S, clock: C, health: H) -> Self {
        Self {
            spawner,
            clock,
            health,
            last_attempt: None,
        }
    }

    /// Make sure a hub is serving, if this machine has one to serve.
    ///
    /// A spawn — successful or refused — records the attempt. [`Launch::Missing`]
    /// does not, so a hub installed a second later starts a second later.
    pub fn ensure(&mut self, hub: Option<&Path>) -> Launch {
        if self.health.is_serving() {
            return Launch::AlreadyServing;
        }
        let Some(hub) = hub else {
            return Launch::Missing;
        };

        let now = self.clock.now();
        if self
            .last_attempt
            .is_some_and(|last| (now - last).unsigned_abs() < THROTTLE)
        {
            return Launch::Throttled;
        }

        self.last_attempt = Some(now);
        match self.spawner.spawn(hub, &SERVE_ARGS) {
            Ok(()) => Launch::Spawned,
            Err(_) => Launch::Failed,
        }
    }
}

/// The clock the binary injects.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

/// The health probe the binary injects: one `GET /v1/health`.
pub struct HttpHealth {
    hub: Hub,
}

impl HttpHealth {
    pub fn new(hub: Hub) -> Self {
        Self { hub }
    }
}

impl HealthProbe for HttpHealth {
    fn is_serving(&self) -> bool {
        self.hub.get(HEALTH_PATH).is_ok()
    }
}

/// The spawner the binary injects.
///
/// The hub is started with its streams closed so it can outlive this process
/// without holding tmux's pane open. It is *not* put in a session of its own:
/// that needs `setsid`, and `unsafe` is forbidden workspace-wide. Nothing here
/// depends on it — a hub that dies with its terminal is simply started again by
/// the next surface, which is the same fail-open path as a hub that never ran.
pub struct DetachedSpawner {
    /// The hub this process started, kept only to be reaped.
    started: Option<Child>,
}

impl DetachedSpawner {
    pub fn new() -> Self {
        Self { started: None }
    }
}

impl Default for DetachedSpawner {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessSpawner for DetachedSpawner {
    fn spawn(&mut self, program: &Path, args: &[&str]) -> Result<(), SpawnError> {
        // A hub we started and that has since exited would otherwise sit in the
        // process table until this surface exits.
        if let Some(started) = self.started.as_mut() {
            if matches!(started.try_wait(), Ok(Some(_))) {
                self.started = None;
            }
        }

        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // Redirecting stdio does not prevent Windows from allocating a console
        // for the hub, which is a console executable started by a GUI surface.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let child = command
            .spawn()
            .map_err(|err| SpawnError::new(err.to_string()))?;
        self.started = Some(child);
        Ok(())
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[test]
    fn detached_console_child_has_no_console_window() {
        let powershell = std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let script = r#"Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public class ConsoleProbe { [DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow(); }'; if ([ConsoleProbe]::GetConsoleWindow() -ne [IntPtr]::Zero) { exit 42 }; exit 0"#;
        let mut spawner = DetachedSpawner::new();
        spawner
            .spawn(
                &powershell,
                &["-NoProfile", "-NonInteractive", "-Command", script],
            )
            .unwrap();
        let child = spawner.started.as_mut().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(
                    status.success(),
                    "child allocated a console or probe failed: {status}"
                );
                break;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("console probe timed out");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
