//! Is that process still there?
//!
//! A closed terminal never sends SessionEnd, so without this a job would sit in
//! every surface forever. The question is the same on both platforms and the
//! bias is the same too: **only a definite "no such process" counts as dead.**
//! A pid we may not query is still running, and an answer we do not understand
//! is not evidence of death — closing a live agent's row would be far worse
//! than leaving a dead one on screen until its next report.
//!
//! Callers inject this behind their own trait (`nerve-hub`'s `PidProbe`) so the
//! state machines stay testable without an OS.

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as sys;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as sys;

/// What a probe can say about a pid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PidState {
    /// The process exists.
    Alive,
    /// No such process.
    Dead,
    /// It exists but will not answer us — `EPERM` on unix,
    /// `ERROR_ACCESS_DENIED` on Windows. Still alive.
    Denied,
}

/// Ask the OS about `pid`.
pub fn state(pid: i32) -> PidState {
    #[cfg(any(unix, windows))]
    {
        sys::state(pid)
    }
    // No probe for this target: report the fail-open answer rather than
    // refusing to build. Reaping stops; nothing else changes.
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        PidState::Denied
    }
}

/// Opaque identity of when `pid` started.
///
/// [`state`] answers alive for a recycled pid. Comparing this token with the
/// one recorded when the job reported the pid tells those apart. `None` means
/// the OS would not say — that is not evidence of death. `pid <= 1` is not a
/// producer and is not probed.
pub fn start_token(pid: i32) -> Option<String> {
    if pid <= 1 {
        return None;
    }
    #[cfg(any(unix, windows))]
    {
        sys::start_token(pid)
    }
    // No probe for this target: same fail-open answer as [`state`].
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        None
    }
}

/// The boolean most callers want: anything but a definite "gone".
pub fn is_alive(pid: i32) -> bool {
    !matches!(state(pid), PidState::Dead)
}
