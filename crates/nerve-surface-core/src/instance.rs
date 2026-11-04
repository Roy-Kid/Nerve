//! One surface per machine.
//!
//! What the lock protects is the hub's refcount. A second surface that also
//! opened a stream would hold the hub alive after the first one left
//! (CLAUDE.md invariant 4), so a process that does not own the lock must never
//! open one.
//!
//! Where the lock *lives* is the surface's business, not this module's: the
//! tmux sidebar keeps it in a tmux user option, which dies with the server it
//! belongs to and leaves nothing on disk; another surface may
//! reasonably use a registry key or a bound port. [`LockStore`] is that seam,
//! and the ownership rule below is the same either way.

use nerve_platform::pid;

/// The key a surface records its pid under.
pub const SURFACE_PID_KEY: &str = "@nerve_surface_pid";

/// Somewhere a single string survives for as long as the surface should.
pub trait LockStore {
    /// How this particular store fails — a tmux error, an I/O error, whatever
    /// the surface already has.
    type Error;

    fn get(&mut self, key: &str) -> Result<Option<String>, Self::Error>;
    fn set(&mut self, key: &str, value: &str) -> Result<(), Self::Error>;
}

/// Is that process still there?
pub trait PidProbe {
    fn is_alive(&self, pid: i32) -> bool;
}

/// The probe a binary injects: the real OS.
///
/// Anything other than "no such process" counts as alive — a pid we may not
/// query is still running, and a second surface painting over the first would
/// be worse than a stale lock nobody reads.
pub struct SystemPidProbe;

impl PidProbe for SystemPidProbe {
    fn is_alive(&self, pid: i32) -> bool {
        pid::is_alive(pid)
    }
}

/// The answer to "may this process be the surface?".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Claim {
    /// This process owns the surface; the lock now names our pid.
    Owned,
    /// A live surface already holds it: exit 0 having opened nothing.
    Yield { pid: i32 },
}

impl Claim {
    /// Whether the caller may open the SSE connection.
    pub fn may_stream(self) -> bool {
        matches!(self, Self::Owned)
    }
}

/// Takes, or stands down from, the single-surface lock.
pub struct InstanceGuard<S: LockStore, P: PidProbe> {
    store: S,
    probe: P,
    me: i32,
}

impl<S: LockStore, P: PidProbe> InstanceGuard<S, P> {
    /// `me` is this process's pid, injected rather than read, so the guard is a
    /// unit test rather than a process.
    pub fn new(store: S, probe: P, me: i32) -> Self {
        Self { store, probe, me }
    }

    /// Read the lock and either take it or stand down.
    ///
    /// A yielding surface never rewrites the lock: the pid recorded there
    /// belongs to the process that is doing the work.
    pub fn claim(&mut self) -> Result<Claim, S::Error> {
        let stored = self.store.get(SURFACE_PID_KEY)?;
        let owner = stored.as_deref().and_then(Self::pid);

        if let Some(pid) = owner
            && pid != self.me
            && self.probe.is_alive(pid)
        {
            return Ok(Claim::Yield { pid });
        }
        self.store.set(SURFACE_PID_KEY, &self.me.to_string())?;
        Ok(Claim::Owned)
    }

    /// A stored value that could name a running surface.
    ///
    /// `pid <= 1` is never an owner — 0 is "no process" and 1 is init — the
    /// same rule the hub applies to producer pids
    /// (`crates/nerve-hub/src/model/job.rs:136`).
    fn pid(stored: &str) -> Option<i32> {
        stored.trim().parse::<i32>().ok().filter(|pid| *pid > 1)
    }
}
