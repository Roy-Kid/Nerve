//! Local rows whose producer process is gone.
//!
//! A closed terminal never sends SessionEnd, so a job would otherwise sit in
//! the panel forever. The probe is injected: process liveness is an OS fact,
//! and the state machine must stay testable without one
//! (`SubjectStore.swift:641`).
//!
//! `kill(pid, 0)` stays true after the kernel gives that pid to someone else.
//! The store records a start token beside `extensions.pid` when the producer
//! checks in; a different token means that process is gone. A missing token
//! is not an exit, and neither is a row that has gone quiet.

use std::sync::Arc;

use crate::model::{Job, WireTime};

/// A job younger than this is never reaped: a producer may not have published
/// its pid yet right after spawn (`SubjectStore.swift:645`).
pub const MIN_REAP_AGE_SECS: i64 = 3;

pub use nerve_platform::pid::PidState;

/// Liveness oracle for producer processes on this machine.
pub trait PidProbe: Send + Sync {
    fn state(&self, pid: i32) -> PidState;

    /// Opaque start token of `pid`, if the OS will say.
    ///
    /// `None` is not death. A missing token must not close a row.
    fn start(&self, _pid: i32) -> Option<String> {
        None
    }
}

/// The probe the daemon injects — `kill(pid, 0)` on unix, `OpenProcess` on
/// Windows, plus the process start token when the OS will give one
/// (`SubjectStore.swift:682`).
///
/// Neither call signals the agent. Only a definite "no such process", or a
/// start token that no longer matches the one recorded for that pid, counts
/// as dead. A pid we may not query (`EPERM`) is still running, and an answer
/// we do not understand is not evidence of death.
pub struct SignalProbe;

impl PidProbe for SignalProbe {
    fn state(&self, pid: i32) -> PidState {
        nerve_platform::pid::state(pid)
    }

    fn start(&self, pid: i32) -> Option<String> {
        nerve_platform::pid::start_token(pid)
    }
}

/// A job the reaper wants closed, with the summary to record on it.
#[derive(Clone, Debug)]
pub struct Victim {
    pub job_id: String,
    pub summary: String,
}

/// Decides which local jobs have outlived their producer.
///
/// Decision only — closing a job is the store's job, because the store is the
/// single write point.
pub struct Reaper {
    probe: Arc<dyn PidProbe>,
    min_age_secs: i64,
}

impl Reaper {
    pub fn new(probe: Arc<dyn PidProbe>) -> Self {
        Self {
            probe,
            min_age_secs: MIN_REAP_AGE_SECS,
        }
    }

    /// Start token of `pid`, if the OS will say. `pid <= 1` is never probed.
    /// An empty answer is the same as no answer: not evidence of death.
    pub(crate) fn start_of(&self, pid: i32) -> Option<String> {
        if pid <= 1 {
            return None;
        }
        self.probe.start(pid).filter(|token| !token.is_empty())
    }

    /// Every job that should be closed as `process_gone` right now.
    pub fn victims<'a>(
        &self,
        jobs: impl Iterator<Item = &'a Job>,
        now: WireTime,
        machine_alias: &str,
    ) -> Vec<Victim> {
        jobs.filter_map(|job| self.victim(job, now, machine_alias))
            .collect()
    }

    fn victim(&self, job: &Job, now: WireTime, _machine_alias: &str) -> Option<Victim> {
        if job.is_ended() || !job.is_conversation_job() {
            return None;
        }
        // Only local-authority jobs enter this store; remote snapshots are separate.
        // Younger than the minimum stays even if the pid already looks recycled:
        // the producer may still be publishing the pid it just spawned.
        if now.seconds_since(job.created_at) < self.min_age_secs {
            return None;
        }
        // `pid <= 1` is not a producer and is never probed. No pid at all is
        // not an exit either — silence is not a closed process.
        let pid = job.producer_pid()?;
        let gone = match self.probe.state(pid) {
            PidState::Dead => true,
            // EPERM: the process exists, we just cannot signal it.
            PidState::Denied => false,
            PidState::Alive => self.start_changed(job, pid),
        };
        if !gone {
            return None;
        }
        Some(Victim {
            job_id: job.id.clone(),
            summary: format!("Process gone (pid {pid})"),
        })
    }

    /// `true` only when the token recorded for this pid is not the process
    /// wearing it now. A missing token on either side is not death.
    fn start_changed(&self, job: &Job, pid: i32) -> bool {
        let Some(recorded) = job.pid_start() else {
            return false;
        };
        self.start_of(pid)
            .is_some_and(|current| current != recorded)
    }
}
