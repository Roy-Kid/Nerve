//! Local rows whose producer process is gone.
//!
//! A closed terminal never sends SessionEnd, so a job would otherwise sit in
//! the panel forever. The probe is injected: process liveness is an OS fact,
//! and the state machine must stay testable without one
//! (`SubjectStore.swift:641`).

use std::sync::Arc;

use crate::model::{Job, WireTime};

/// A job younger than this is never reaped: a producer may not have published
/// its pid yet right after spawn (`SubjectStore.swift:645`).
pub const MIN_REAP_AGE_SECS: i64 = 3;

pub use nerve_platform::pid::PidState;

/// Liveness oracle for producer processes on this machine.
pub trait PidProbe: Send + Sync {
    fn state(&self, pid: i32) -> PidState;
}

/// The probe the daemon injects — `kill(pid, 0)` on unix, `OpenProcess` on
/// Windows (`SubjectStore.swift:682`).
///
/// Whichever it is, only a definite "no such process" counts as dead: a pid we
/// may not query is still running, and an answer we do not understand is not
/// evidence of death. Closing a live agent's row would be far worse than
/// leaving a dead one on screen until its next report.
pub struct SignalProbe;

impl PidProbe for SignalProbe {
    fn state(&self, pid: i32) -> PidState {
        nerve_platform::pid::state(pid)
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
        if now.seconds_since(job.created_at) < self.min_age_secs {
            return None;
        }
        let pid = job.producer_pid()?;
        if self.probe.state(pid) != PidState::Dead {
            return None;
        }
        Some(Victim {
            job_id: job.id.clone(),
            summary: format!("Process gone (pid {pid})"),
        })
    }
}
