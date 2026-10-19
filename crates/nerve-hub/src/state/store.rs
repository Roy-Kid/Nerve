//! The single write point for every job the hub knows about.
//!
//! Ported from `Nerve/Nerve/Store/SubjectStore.swift`, minus everything that is
//! a surface concern: no ribbon geometry, no status derivation, and no
//! `ensureLocalActions` — producer actions are echoed verbatim, Open/Copy are
//! derived where they are used.
//!
//! Deliberate additions over the Swift original: a `departed` buffer (SessionEnd
//! evicts immediately, so a plain frame diff would lose the terminal state) and
//! the per-job `timeline` published inside each job.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};
use serde_json::Value;

use crate::clock::Clock;
use crate::model::{
    ActionState, Attention, Current, Event, EventKind, Health, Job, Lifecycle, Outcome, WireTime,
};

use super::dedupe::SeenEvents;
use super::demo::DemoRows;
use super::pending::{PendingAction, PendingQueue};
use super::reaper::{PidProbe, Reaper};
use super::timeline::{TimelineEntry, Timelines};

/// Every job, its timeline, and the requests waiting on producers.
///
/// In memory only: jobs, timelines and pending requests never touch disk.
pub struct JobStore {
    clock: Arc<dyn Clock>,
    /// Alias for "this machine" — fills empty job aliases and gates the reaper.
    machine_alias: String,
    reaper: Reaper,
    jobs: BTreeMap<String, Job>,
    timelines: Timelines,
    pending: PendingQueue,
    seen: SeenEvents,
    last_hooks: BTreeMap<(String, String), String>,
    departed: Vec<Job>,
    pub(crate) instance_id: String,
    pub(crate) revision: u64,
    pub(crate) machine_id: String,
    pub(crate) remote_jobs: BTreeMap<String, Vec<Value>>,
    pub(crate) machines: BTreeMap<String, Value>,
}

/// Open fields a snapshot inherits from the job it replaces.
///
/// A hook is one process per event, so only the `UserPromptSubmit` run ever
/// sees the prompt — every snapshot after it would blank the field. Patches
/// already merge per key (`state/patch.rs`); this is the same promise at the
/// snapshot door, kept deliberately narrow: facts *about* the job that a
/// producer states once, never live status (which must be free to go away).
const STICKY_EXTENSIONS: [&str; 2] = ["lastPrompt", "lastPromptAt"];

/// A job as published: its own wire shape plus the timeline the hub keeps for it.
///
/// Borrowed rather than owned, so the frame that publishes a whole job set
/// serialises straight out of the store — no intermediate `serde_json::Value`
/// tree to build and walk a second time.
pub struct JobView<'a> {
    job: &'a Job,
    timeline: &'a [TimelineEntry],
    display: crate::projection::Display,
}

impl Serialize for JobView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut value = serde_json::to_value(self.job).map_err(serde::ser::Error::custom)?;
        value["timeline"] =
            serde_json::to_value(self.timeline).map_err(serde::ser::Error::custom)?;
        value["display"] =
            serde_json::to_value(&self.display).map_err(serde::ser::Error::custom)?;
        let location = self.job.location.clone().unwrap_or_default();
        let mut actions: Vec<_> = self
            .job
            .actions
            .iter()
            .filter(|a| {
                matches!(
                    a.kind.to_ascii_lowercase().as_str(),
                    "open" | "focus" | "open_logs" | "openlogs" | "copy" | "copy_summary"
                )
            })
            .cloned()
            .collect();
        for action in crate::hook::build::local_actions(&location) {
            if !actions
                .iter()
                .any(|a| a.id == action.id || a.kind == action.kind)
            {
                actions.push(action);
            }
        }
        actions.sort_by_key(|a| match a.kind.as_str() {
            "open" | "focus" => 0,
            "copy" | "copy_summary" => 1,
            _ => 2,
        });
        value["actions"] = serde_json::to_value(actions).map_err(serde::ser::Error::custom)?;
        value.serialize(serializer)
    }
}
impl<'a> JobView<'a> {
    /// A row that has left: eviction forgot its timeline, and a surface must
    /// parse one job shape rather than two, so it publishes an empty one.
    pub(crate) fn departed(job: &'a Job) -> Self {
        Self {
            job,
            timeline: &[],
            display: crate::projection::display(job),
        }
    }
}

/// The live job set as a serde sequence, borrowed from the store.
///
/// A frame serialises this instead of collecting a `Vec<JobView>` it would
/// drop immediately after writing.
pub struct JobsSeq<'a>(pub &'a JobStore);

impl Serialize for JobsSeq<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.jobs.len()))?;
        let mut jobs: Vec<_> = self.0.jobs.values().collect();
        jobs.sort_by_key(|job| crate::projection::display(job).order_key);
        for job in jobs {
            seq.serialize_element(&JobView {
                job,
                display: crate::projection::display(job),
                timeline: self.0.timelines.of(&job.id),
            })?;
        }
        seq.end()
    }
}

impl JobStore {
    pub fn new(clock: Arc<dyn Clock>, machine_alias: String, probe: Arc<dyn PidProbe>) -> Self {
        Self {
            clock,
            machine_alias,
            reaper: Reaper::new(probe),
            jobs: BTreeMap::new(),
            timelines: Timelines::default(),
            pending: PendingQueue::default(),
            seen: SeenEvents::default(),
            last_hooks: BTreeMap::new(),
            departed: Vec::new(),
            instance_id: format!(
                "{}-{}",
                std::process::id(),
                time::OffsetDateTime::now_utc().unix_timestamp_nanos()
            ),
            revision: 0,
            machine_id: crate::federation::machine_id(),
            remote_jobs: BTreeMap::new(),
            machines: BTreeMap::new(),
        }
    }

    /// The clock this store was assembled with.
    pub(crate) fn clock(&self) -> &dyn crate::clock::Clock {
        &*self.clock
    }

    // ── Ingest ──────────────────────────────────────────────────────────────

    /// Apply a snapshot batch; returns how many jobs were **seen**.
    ///
    /// Ended and legacy-noise rows count too: the producer reported them, and
    /// `{"applied":N}` answers "how much of your batch did I read".
    pub fn remember_hook(&mut self, producer: &str, id: &str) -> bool {
        self.seen.remember(&format!("hook/{producer}/{id}"))
    }

    pub fn apply_snapshot(&mut self, jobs: Vec<Job>) -> usize {
        let seen = jobs.len();
        for job in jobs {
            self.store_snapshot(job);
        }
        seen
    }

    /// Apply one event; `false` means it changed nothing (duplicate id, stale
    /// version, or an unknown job with nothing to create from).
    pub fn apply_event(&mut self, event: Event) -> bool {
        self.revision += 1;
        if !self.seen.remember(&event.id) {
            return false;
        }
        if let Some(full) = self.body_to_apply(&event) {
            let title = if event.kind == EventKind::Snapshot {
                "Snapshot"
            } else {
                "Created"
            };
            let job_id = full.id.clone();
            self.store_snapshot(full);
            self.timelines.record(
                &job_id,
                event.kind.wire(),
                title.to_string(),
                event.timestamp,
            );
            return true;
        }
        if self.jobs.contains_key(&event.job_id) {
            self.patch_existing(&event)
        } else {
            self.open_on_miss(&event)
        }
    }

    /// The full job body an event carries, when the port applies it as a
    /// snapshot rather than a patch (`SubjectStore.swift:439`).
    fn body_to_apply(&self, event: &Event) -> Option<Job> {
        let full = event.job.clone()?;
        let as_snapshot = event.kind == EventKind::Snapshot
            || event.kind == EventKind::JobCreated
            || !self.jobs.contains_key(&event.job_id);
        as_snapshot.then_some(full)
    }

    fn store_snapshot(&mut self, mut job: Job) {
        self.revision += 1;
        // Job-level alias fallback, after envelope-level normalisation.
        if job.alias.is_empty() {
            job.alias.clone_from(&self.machine_alias);
        }
        if !job.is_conversation_job() {
            self.drop_legacy_child(&job.id);
            return;
        }
        if let Some(stored) = self.jobs.get(&job.id) {
            if job.version < stored.version {
                return;
            }
            // Tool chatter rewrites the snapshot. Keep the anchors the panel
            // sorts on, the same way `sealed` keeps them when a job ends.
            job.created_at = stored.created_at;
            job.started_at = Some(stored.started_at.unwrap_or(stored.created_at));
            for key in STICKY_EXTENSIONS {
                if job.extensions.contains_key(key) {
                    continue;
                }
                if let Some(value) = stored.extensions.get(key) {
                    job.extensions.insert(key.to_string(), value.clone());
                }
            }
        }
        if job.is_ended() {
            let ended = self.sealed(job);
            self.evict_ended(ended);
            return;
        }
        let id = job.id.clone();
        self.jobs.insert(id.clone(), job);
        // A parent update also scrubs leftover child rows for that session.
        self.purge_legacy_children(&id);
    }

    fn patch_existing(&mut self, event: &Event) -> bool {
        let Some(stored) = self.jobs.get(&event.job_id) else {
            return false;
        };
        if event
            .version
            .is_some_and(|version| version < stored.version)
        {
            return false;
        }
        let mut next = stored.patched(event);
        next.version = match event.version {
            Some(version) => next.version.max(version),
            None => next.version.saturating_add(1),
        };
        next.updated_at = next.updated_at.max(event.timestamp);
        self.keep_or_evict(next, event)
    }

    /// Create-on-miss: only a `job.created` event, or one carrying a name, may
    /// open a job that the hub has never heard of (`SubjectStore.swift:462`).
    fn open_on_miss(&mut self, event: &Event) -> bool {
        if event.kind != EventKind::JobCreated && event.name.is_none() {
            return false;
        }
        let opened = Job::opened_by(event, &self.machine_alias);
        let next = opened.patched(event);
        self.keep_or_evict(next, event)
    }

    /// Store the patched job, or let it leave if the event closed it. A closed
    /// session is never inserted into the panel first.
    fn keep_or_evict(&mut self, next: Job, event: &Event) -> bool {
        if next.is_ended() {
            self.evict_ended(next);
            return true;
        }
        self.timelines.record(
            &next.id,
            event.kind.wire(),
            event.timeline_title(),
            event.timestamp,
        );
        self.jobs.insert(next.id.clone(), next);
        true
    }

    // ── Leaving ─────────────────────────────────────────────────────────────

    /// A closing job with its stored duration anchors kept, and `endedAt`
    /// filled from the injected clock when the producer left it out
    /// (`SubjectStore.swift:530`).
    fn sealed(&self, job: Job) -> Job {
        let mut ended = job;
        if let Some(stored) = self.jobs.get(&ended.id) {
            ended.created_at = stored.created_at;
            ended.started_at = Some(stored.started_at.unwrap_or(stored.created_at));
        }
        if ended.ended_at.is_none() {
            ended.ended_at = Some(self.now());
        }
        ended
    }

    /// Session closed: keep the terminal state for subscribers, then drop the
    /// row. There is no Success linger — only open work stays visible.
    fn evict_ended(&mut self, ended: Job) {
        self.revision += 1;
        let id = ended.id.clone();
        self.jobs.remove(&id);
        self.timelines.forget(&id);
        // Legacy children of this conversation leave with their parent.
        self.purge_legacy_children(&id);
        self.departed.push(ended);
    }

    /// Terminal states since the last call, drained.
    pub fn take_departed(&mut self) -> Vec<Job> {
        std::mem::take(&mut self.departed)
    }

    /// The same terminal states, drained and deduplicated.
    ///
    /// One rule the typed drain above does not carry, and it belongs to the
    /// frame that publishes them: one id appears at most once — a producer that
    /// reports the end twice inside a window leaves only its last word.
    pub(crate) fn drain_departed_deduped(&mut self) -> Vec<Job> {
        let mut last: BTreeMap<String, Job> = BTreeMap::new();
        for job in self.take_departed() {
            last.insert(job.id.clone(), job);
        }
        last.into_values().collect()
    }

    fn drop_legacy_child(&mut self, id: &str) {
        self.jobs.remove(id);
        self.timelines.forget(id);
    }

    fn purge_legacy_children(&mut self, parent_id: &str) {
        let children: Vec<String> = self
            .jobs
            .values()
            .filter(|job| !job.is_conversation_job() && job.has_legacy_parent(parent_id))
            .map(|job| job.id.clone())
            .collect();
        for id in children {
            self.drop_legacy_child(&id);
        }
    }

    /// Local policy closes a row (dead pid today). Producers are not told —
    /// the hub never signals an agent.
    fn end_locally(&mut self, id: &str, reason: &str, summary: &str, outcome: Outcome) {
        let now = self.now();
        let Some(stored) = self.jobs.get(id) else {
            return;
        };
        if stored.is_ended() {
            return;
        }
        let mut ended = stored.clone();
        ended.lifecycle = Lifecycle::Ended;
        ended.outcome = Some(outcome);
        ended.ended_at = Some(now);
        ended.updated_at = now;
        ended.version = ended.version.saturating_add(1);
        ended.attention = Attention::default();
        ended.health = Health::Ok;
        ended.current = Some(Current {
            kind: "idle".to_string(),
            name: None,
            summary: Some(summary.to_string()),
            detail: None,
            started_at: None,
        });
        ended
            .extensions
            .insert("endReason".to_string(), Value::String(reason.to_string()));
        self.evict_ended(ended);
    }

    // ── Maintenance ─────────────────────────────────────────────────────────

    /// Periodic tick: expire stale requests, then reap dead local producers
    /// (`SubjectStore.swift:1021`).
    ///
    /// Reports whether it changed anything, so a caller driving it on a timer
    /// can tell a quiet tick from one that closed a row — the hub publishes a
    /// frame for the second kind only.
    pub fn expire_and_reap(&mut self) -> bool {
        let expired = self.expire_pending();
        let reaped = self.reap_dead_local_producers();
        if expired || reaped {
            self.revision += 1;
        }
        expired || reaped
    }

    /// Mark every request past its TTL expired and re-state the actions they
    /// referred to. `GET /v1/actions/pending` runs this before it filters
    /// (`IngestServer.swift:178`), so a stale request is never handed out.
    pub fn expire_pending(&mut self) -> bool {
        let now = self.now();
        let expired = self.pending.expire(now);
        let changed = !expired.is_empty();
        for reference in expired {
            self.set_action_state(
                &reference.job_id,
                &reference.action_id,
                ActionState::Expired,
            );
        }
        changed
    }

    fn reap_dead_local_producers(&mut self) -> bool {
        let now = self.now();
        let victims = self
            .reaper
            .victims(self.jobs.values(), now, &self.machine_alias);
        let reaped = !victims.is_empty();
        for victim in victims {
            self.end_locally(
                &victim.job_id,
                "process_gone",
                &victim.summary,
                Outcome::Cancelled,
            );
        }
        reaped
    }

    /// Seed the demo rows (`SubjectStore.swift:1090`).
    ///
    /// They enter through the same door as any reported job, so a demo row is
    /// subject to every rule a real one is — including this machine's alias and
    /// the legacy-noise filter.
    pub fn load_demo(&mut self) {
        let now = self.now();
        for job in DemoRows::new(self.machine_alias.clone(), now).jobs() {
            let id = job.id.clone();
            self.store_snapshot(job);
            self.timelines
                .record(&id, "snapshot", "Demo loaded".to_string(), now);
        }
    }

    /// Admin wipe. Not a lifecycle transition, so nothing departs.
    pub fn clear(&mut self) {
        self.revision += 1;
        self.jobs.clear();
        self.departed.clear();
        self.timelines.clear();
        self.pending.clear();
        self.seen.clear();
        self.last_hooks.clear();
    }

    // ── Pending actions (dormant — nothing enqueues today) ───────────────────

    /// Bind a request to a declared action. `false` when the job or the action
    /// is unknown: the hub never invents an action.
    pub fn enqueue_pending(&mut self, id: &str, job_id: &str, action_id: &str) -> bool {
        let Some((producer_id, action_kind, title)) = self.declared_action(job_id, action_id)
        else {
            return false;
        };
        let now = self.now();
        self.pending.enqueue(PendingAction {
            id: id.to_string(),
            job_id: job_id.to_string(),
            producer_id,
            action_id: action_id.to_string(),
            action_kind,
            title: title.clone(),
            requested_at: now,
            state: ActionState::Pending,
            result_message: None,
            expires_at: None,
        });
        self.set_action_state(job_id, action_id, ActionState::Pending);
        self.timelines
            .record(job_id, "action.pending", format!("{title} → source"), now);
        true
    }

    /// Open requests as a bare JSON array, optionally for one producer.
    pub fn pending_json(&self, producer_id: Option<&str>) -> Value {
        let open = self.pending.open(self.now(), producer_id);
        // Plain data: the fallback is unreachable, and an empty array is the
        // only answer that still honours "this endpoint returns an array".
        serde_json::to_value(open).unwrap_or_else(|_| Value::Array(Vec::new()))
    }

    /// A producer reports how its action ended.
    ///
    /// Swift defers the re-arm to the main queue; the hub has no run loop, so a
    /// succeeded/failed action returns to `available` inline — re-usable rather
    /// than frozen (`SubjectStore.swift:984`).
    pub fn complete_pending(
        &mut self,
        id: &str,
        state: ActionState,
        message: Option<&str>,
        producer_id: Option<&str>,
    ) -> bool {
        let Some(done) = self.pending.complete(id, state, message, producer_id) else {
            return false;
        };
        // An expired action stays expired; a succeeded/failed one re-arms so the
        // row is usable again if the producer re-declares it.
        let settled = match state {
            ActionState::Expired => ActionState::Expired,
            _ => ActionState::Available,
        };
        self.set_action_state(&done.job_id, &done.action_id, settled);
        let title = message.map_or_else(
            || format!("{}: {}", done.title, state.wire()),
            str::to_string,
        );
        let now = self.now();
        self.timelines
            .record(&done.job_id, "action.completed", title, now);
        true
    }

    fn declared_action(&self, job_id: &str, action_id: &str) -> Option<(String, String, String)> {
        let job = self.jobs.get(job_id)?;
        let action = job.actions.iter().find(|action| action.id == action_id)?;
        Some((
            job.producer.id.clone(),
            action.kind.clone(),
            action.title.clone(),
        ))
    }

    fn set_action_state(&mut self, job_id: &str, action_id: &str, state: ActionState) {
        let now = self.now();
        let Some(job) = self.jobs.get_mut(job_id) else {
            return;
        };
        let Some(action) = job.actions.iter_mut().find(|action| action.id == action_id) else {
            return;
        };
        action.state = state;
        job.updated_at = now;
        self.revision += 1;
    }

    // ── Publication ─────────────────────────────────────────────────────────

    /// Live conversation rows for this producer in this workspace.
    ///
    /// Used by hook ingest to close a superseded session when the in-memory
    /// slot map was empty (hub restart) or the new session never sent
    /// `SessionStart`.
    pub fn live_ids_in_workspace(&self, producer_id: &str, workspace: &str) -> Vec<String> {
        if workspace.is_empty() {
            return Vec::new();
        }
        self.jobs
            .values()
            .filter(|job| {
                job.producer.id == producer_id
                    && job
                        .context
                        .as_ref()
                        .and_then(|context| context.workspace.as_deref())
                        == Some(workspace)
            })
            .map(|job| job.id.clone())
            .collect()
    }

    /// Stored UI slot for `id`, when the row recorded one.
    pub fn stored_slot(&self, id: &str) -> Option<&str> {
        let text = self.jobs.get(id)?.extensions.get("slot")?.as_str()?;
        let text = text.trim();
        if text.is_empty() { None } else { Some(text) }
    }

    /// Every stored job as a bare JSON array, each with its timeline.
    ///
    /// Same [`JobsSeq`] walk as an SSE frame — no intermediate `Vec<JobView>`.
    pub fn jobs_json(&self) -> Value {
        let mut rows = self
            .local_jobs_json()
            .as_array()
            .cloned()
            .unwrap_or_default();
        for jobs in self.remote_jobs.values() {
            rows.extend(jobs.iter().cloned());
        }
        rows.sort_by(|a, b| {
            a["display"]["orderKey"]
                .as_str()
                .cmp(&b["display"]["orderKey"].as_str())
                .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
        });
        for (index, row) in rows.iter_mut().enumerate() {
            let online = row["machineId"].as_str() == Some(&self.machine_id)
                || self
                    .machines
                    .values()
                    .any(|m| m["machineId"] == row["machineId"] && m["state"] == "connected");
            row["display"]["sourceOnline"] = serde_json::json!(online);
            row["display"]["machineGroup"] = row["machineId"].clone();
            if !online {
                let label = row["display"]["statusLabel"].as_str().unwrap_or("Task");
                row["display"]["statusLabel"] = serde_json::json!(format!("{label} · Offline"));
            }
            row["display"]["order"] = serde_json::json!(index);
            row["extensions"]["hub"] = row["display"].clone();
        }
        Value::Array(rows)
    }

    pub fn remember_raw_hook(&mut self, producer: &str, session: &str, payload: &Value) -> bool {
        let key = (producer.to_owned(), session.to_owned());
        let raw = payload.to_string();
        if self.last_hooks.get(&key) == Some(&raw) {
            return false;
        }
        if self.last_hooks.len() >= 200 {
            self.last_hooks.pop_first();
        }
        self.last_hooks.insert(key, raw);
        true
    }
    pub fn local_jobs_json(&self) -> Value {
        let mut rows =
            serde_json::to_value(JobsSeq(self)).unwrap_or_else(|_| Value::Array(Vec::new()));
        if let Some(jobs) = rows.as_array_mut() {
            for job in jobs {
                job["machineId"] = Value::String(self.machine_id.clone());
                job["originJobId"] = job["id"].clone();
                job["display"]["machineLabel"] = serde_json::json!(self.machine_alias);
            }
        }
        rows
    }

    fn now(&self) -> WireTime {
        WireTime::new(self.clock.now())
    }
}
