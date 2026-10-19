//! State-core contract tests (spec `nerve-hub` · T2 · acceptance A4 + A8).
//!
//! RED on purpose: nothing below exists yet. T3 implements `model/` + `state/`
//! against the contract in this header; these tests are the API definition.
//!
//! ─────────────────────────────────────────────────────────────────────────
//! MINIMAL API CONTRACT (what T3 must expose)
//! ─────────────────────────────────────────────────────────────────────────
//!
//! ```text
//! nerve_hub::model
//!   pub struct Job      // serde, camelCase wire names, Clone + Debug
//!   pub struct Event    // serde, camelCase wire names, Clone + Debug
//!   pub struct Envelope // {alias?, machineKind?, jobs?, events?} — declared
//!                       // for T4/T5 routes; NOT exercised in this file.
//!   pub enum ActionState { Available, Pending, Succeeded, Failed, Expired, Unsupported }
//!                       // serde rename_all = "lowercase"
//!
//! nerve_hub::state
//!   pub const MAX_SEEN_EVENTS: usize          // 4000 (see `SubjectStore.swift:26`)
//!   pub const SEEN_EVENT_EVICT_BATCH: usize   // 500  (see `SubjectStore.swift:860`)
//!
//!   #[derive(Clone, Copy, Debug, PartialEq, Eq)]
//!   pub enum PidState { Alive, Dead, Denied }  // Denied == EPERM: exists, unsignalable
//!
//!   pub trait PidProbe: Send + Sync {
//!       fn state(&self, pid: i32) -> PidState;
//!   }
//!
//!   pub struct JobStore;
//!   impl JobStore {
//!       pub fn new(
//!           clock: Arc<dyn nerve_hub::clock::Clock>,
//!           machine_alias: String,
//!           probe: Arc<dyn PidProbe>,
//!       ) -> Self;
//!
//!       pub fn apply_snapshot(&mut self, jobs: Vec<Job>) -> usize;
//!       pub fn apply_event(&mut self, event: Event) -> bool;
//!       pub fn jobs_json(&self) -> serde_json::Value;
//!       pub fn take_departed(&mut self) -> Vec<Job>;
//!       pub fn expire_and_reap(&mut self);
//!       pub fn clear(&mut self);
//!
//!       // Dormant pending subsystem (spec D6): ported + testable, no route
//!       // enqueues today. `id` is caller-supplied so the hub needs no uuid dep.
//!       pub fn enqueue_pending(&mut self, id: &str, job_id: &str, action_id: &str) -> bool;
//!       pub fn pending_json(&self, producer_id: Option<&str>) -> serde_json::Value;
//!       pub fn complete_pending(
//!           &mut self,
//!           id: &str,
//!           state: ActionState,
//!           message: Option<&str>,
//!           producer_id: Option<&str>,
//!       ) -> bool;
//!   }
//! ```
//!
//! Semantics the signatures do not carry (ported from `SubjectStore.swift`):
//!
//! * `apply_snapshot` returns the number of jobs **seen**, not stored — ended
//!   and legacy-noise rows still count (`./scripts/nerve.sh --verify-loop` expects `applied:5`).
//! * `jobs_json` is a **bare array**; every element is the job's wire JSON plus
//!   a hub-maintained `"timeline"` array (always present, newest first, ≤ 40,
//!   entries `{id, jobId, kind, title, timestamp}` — `id` opaque, no uuid dep).
//! * Wire dates are RFC3339 **second precision, no fraction, `Z`** (matches
//!   `nerve_hook.py:103` and Swift's `.iso8601` strategy); `version` is a `u64`
//!   millisecond stamp (`nerve_hook.py:107`).
//! * Job decoding requires `id`, `createdAt`, `updatedAt`, `version` (the hook
//!   always sends them, and requiring them keeps T4's envelope fallback chain
//!   honest); the remaining fields may carry serde defaults. Event decoding
//!   requires `id`, `jobId`, `kind`, `timestamp`, `producerId`.
//! * `take_departed` drains: each evicted job's **terminal state**, once.
//! * `expire_and_reap` is the maintenance tick — expire pending, then reap dead
//!   local producer PIDs (`SubjectStore.swift:1021`).
//! * `complete_pending` accepts only `Succeeded | Failed | Expired`; any other
//!   state returns `false` (`SubjectStore.swift:979`). Swift defers the
//!   re-arm to the main queue; the hub has no run loop, so the reset to
//!   `available` happens inline — the observable end state is the contract.
//! * `ensureLocalActions` is deliberately **not** ported: producer actions are
//!   echoed verbatim (spec "刻意分歧").
//!
//! Determinism: fake clock + fake `PidProbe`, hard-coded goldens, no wall
//! clock, no network, no filesystem.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use time::macros::datetime;

use nerve_hub::clock::FakeClock;
use nerve_hub::model::{ActionState, Event, Job};
use nerve_hub::state::{JobStore, MAX_SEEN_EVENTS, PidProbe, PidState, SEEN_EVENT_EVICT_BATCH};

// ── Fixtures ────────────────────────────────────────────────────────────────

/// Wire instant every fixture is anchored to (same value `./scripts/nerve.sh --verify-loop` uses).
const T0: &str = "2026-07-19T00:00:00Z";

/// The alias injected into `JobStore::new` — "this machine" for reaper rules.
const MACHINE_ALIAS: &str = "test-mac";

struct Fixture {
    store: JobStore,
    clock: Arc<FakeClock>,
    probe: Arc<FakeProbe>,
}

/// Store whose clock reads `T0` and whose probe calls every pid alive.
fn fixture() -> Fixture {
    fixture_with_default_pid(PidState::Alive)
}

fn fixture_with_default_pid(default: PidState) -> Fixture {
    let clock = Arc::new(FakeClock::new(datetime!(2026-07-19 00:00:00 UTC)));
    let probe = Arc::new(FakeProbe::new(default));
    let store = JobStore::new(clock.clone(), MACHINE_ALIAS.to_string(), probe.clone());
    Fixture {
        store,
        clock,
        probe,
    }
}

/// Fake `PidProbe`: a default verdict plus per-pid overrides.
struct FakeProbe {
    default: PidState,
    overrides: Mutex<HashMap<i32, PidState>>,
}

impl FakeProbe {
    fn new(default: PidState) -> Self {
        Self {
            default,
            overrides: Mutex::new(HashMap::new()),
        }
    }

    fn set(&self, pid: i32, state: PidState) {
        self.lock().insert(pid, state);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<i32, PidState>> {
        self.overrides
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl PidProbe for FakeProbe {
    fn state(&self, pid: i32) -> PidState {
        self.lock().get(&pid).copied().unwrap_or(self.default)
    }
}

/// Full wire job (every field the Swift model requires) with `overrides` applied
/// on top, so each test states only the fields it is about.
fn job_fixture(id: &str, overrides: Value) -> Job {
    let base = json!({
        "id": id,
        "kind": "session",
        "name": id,
        "alias": MACHINE_ALIAS,
        "lifecycle": "active",
        "attention": { "level": "none" },
        "health": "ok",
        "progress": { "kind": "none" },
        "producer": { "id": "t" },
        "capabilities": [],
        "actions": [],
        "createdAt": T0,
        "startedAt": T0,
        "updatedAt": T0,
        "version": 1,
        "extensions": {}
    });
    serde_json::from_value(merged(base, overrides)).expect("job fixture must decode")
}

fn event_fixture(id: &str, job_id: &str, kind: &str, overrides: Value) -> Event {
    let base = json!({
        "id": id,
        "jobId": job_id,
        "kind": kind,
        "timestamp": T0,
        "producerId": "t"
    });
    serde_json::from_value(merged(base, overrides)).expect("event fixture must decode")
}

/// Shallow key overwrite — fixtures are flat enough that nested merging would
/// only hide which object a test actually asserts on.
fn merged(base: Value, overrides: Value) -> Value {
    let (Value::Object(mut base), Value::Object(overrides)) = (base, overrides) else {
        panic!("fixture and overrides must both be JSON objects");
    };
    for (key, value) in overrides {
        base.insert(key, value);
    }
    Value::Object(base)
}

// ── Assertion helpers ───────────────────────────────────────────────────────

fn at<'a>(value: &'a Value, pointer: &str) -> &'a Value {
    value
        .pointer(pointer)
        .unwrap_or_else(|| panic!("missing `{pointer}` in {value}"))
}

fn text(value: &Value, pointer: &str) -> String {
    at(value, pointer)
        .as_str()
        .unwrap_or_else(|| panic!("`{pointer}` is not a string in {value}"))
        .to_string()
}

fn number(value: &Value, pointer: &str) -> u64 {
    at(value, pointer)
        .as_u64()
        .unwrap_or_else(|| panic!("`{pointer}` is not a u64 in {value}"))
}

fn array(value: &Value) -> &Vec<Value> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("expected a bare JSON array, got {value}"))
}

fn job_in(jobs: &Value, id: &str) -> Value {
    array(jobs)
        .iter()
        .find(|job| job.get("id").and_then(Value::as_str) == Some(id))
        .cloned()
        .unwrap_or_else(|| panic!("job `{id}` missing from {jobs}"))
}

fn has_job(jobs: &Value, id: &str) -> bool {
    array(jobs)
        .iter()
        .any(|job| job.get("id").and_then(Value::as_str) == Some(id))
}

fn timeline_of(jobs: &Value, id: &str) -> Vec<Value> {
    let job = job_in(jobs, id);
    array(at(&job, "/timeline")).clone()
}

fn wire(job: &Job) -> Value {
    serde_json::to_value(job).expect("job must serialize")
}

// ── 1. Serde: wire fidelity ─────────────────────────────────────────────────

#[test]
fn test_job_dates_roundtrip_at_second_precision_utc() {
    let job = job_fixture(
        "claude-code:s1",
        json!({
            "createdAt": "2026-07-19T00:00:00Z",
            "startedAt": "2026-07-19T07:00:00Z",
            "updatedAt": "2026-07-19T08:10:00Z"
        }),
    );

    let value = wire(&job);

    assert_eq!(text(&value, "/createdAt"), "2026-07-19T00:00:00Z");
    assert_eq!(text(&value, "/startedAt"), "2026-07-19T07:00:00Z");
    assert_eq!(text(&value, "/updatedAt"), "2026-07-19T08:10:00Z");
}

#[test]
fn test_job_version_roundtrips_as_u64_millis() {
    // `nerve_hook.py:107` sends `int(time.time() * 1000)` — past u32 range.
    let job = job_fixture("claude-code:s1", json!({ "version": 1_766_400_000_000u64 }));

    assert_eq!(number(&wire(&job), "/version"), 1_766_400_000_000);

    let mut fixture = fixture();
    fixture.store.apply_snapshot(vec![job]);

    let stored = job_in(&fixture.store.jobs_json(), "claude-code:s1");
    assert_eq!(number(&stored, "/version"), 1_766_400_000_000);
}

// ── 2. Snapshot ordering and counting ───────────────────────────────────────

#[test]
fn test_snapshot_with_older_version_does_not_overwrite() {
    let mut fixture = fixture();
    fixture.store.apply_snapshot(vec![job_fixture(
        "a1",
        json!({ "name": "A", "version": 3 }),
    )]);

    fixture.store.apply_snapshot(vec![job_fixture(
        "a1",
        json!({
            "name": "B",
            "version": 2,
            "attention": { "level": "urgent", "reason": "input" }
        }),
    )]);

    let stored = job_in(&fixture.store.jobs_json(), "a1");
    assert_eq!(text(&stored, "/name"), "A");
    assert_eq!(number(&stored, "/version"), 3);
    assert_eq!(text(&stored, "/attention/level"), "none");
}

#[test]
fn test_last_prompt_survives_the_snapshots_that_follow_it() {
    // The prompt exists in exactly one hook run; every later event posts a
    // snapshot that has never heard of it.
    let mut fixture = fixture();
    fixture.store.apply_snapshot(vec![job_fixture(
        "claude-code:s1",
        json!({
            "version": 1,
            "extensions": { "lastPrompt": "fix the sidebar", "lastPromptAt": T0 }
        }),
    )]);

    fixture.store.apply_snapshot(vec![job_fixture(
        "claude-code:s1",
        json!({ "version": 2, "extensions": { "hookEvent": "PreToolUse" } }),
    )]);

    let stored = job_in(&fixture.store.jobs_json(), "claude-code:s1");
    assert_eq!(text(&stored, "/extensions/lastPrompt"), "fix the sidebar");
    assert_eq!(text(&stored, "/extensions/hookEvent"), "PreToolUse");
}

#[test]
fn test_a_new_prompt_replaces_the_kept_one() {
    let mut fixture = fixture();
    fixture.store.apply_snapshot(vec![job_fixture(
        "claude-code:s1",
        json!({ "version": 1, "extensions": { "lastPrompt": "first" } }),
    )]);
    fixture.store.apply_snapshot(vec![job_fixture(
        "claude-code:s1",
        json!({ "version": 2, "extensions": { "lastPrompt": "second" } }),
    )]);

    let stored = job_in(&fixture.store.jobs_json(), "claude-code:s1");
    assert_eq!(text(&stored, "/extensions/lastPrompt"), "second");
}

#[test]
fn test_live_status_extensions_are_not_sticky() {
    // Only prompt keys carry forward: a snapshot that drops `pid` means the
    // producer stopped reporting one, and the hub must not re-assert it.
    let mut fixture = fixture();
    fixture.store.apply_snapshot(vec![job_fixture(
        "claude-code:s1",
        json!({ "version": 1, "extensions": { "pid": 4242 } }),
    )]);
    fixture.store.apply_snapshot(vec![job_fixture(
        "claude-code:s1",
        json!({ "version": 2, "extensions": {} }),
    )]);

    let stored = job_in(&fixture.store.jobs_json(), "claude-code:s1");
    assert!(stored.pointer("/extensions/pid").is_none());
}

#[test]
fn test_apply_snapshot_counts_every_job_including_ended_and_legacy() {
    let mut fixture = fixture();

    let applied = fixture.store.apply_snapshot(vec![
        job_fixture("a1", json!({})),
        job_fixture("a5", json!({ "lifecycle": "ended", "outcome": "failure" })),
        job_fixture("a1:child", json!({ "extensions": { "parentJobId": "a1" } })),
    ]);

    // `./scripts/nerve.sh --verify-loop` asserts `"applied":5` for a batch whose 5th job is ended.
    assert_eq!(applied, 3);
    assert_eq!(array(&fixture.store.jobs_json()).len(), 1);
}

#[test]
fn test_ended_snapshot_leaves_the_store_and_keeps_anchors() {
    let mut fixture = fixture();
    fixture.store.apply_snapshot(vec![job_fixture(
        "claude-code:s1",
        json!({
            "createdAt": "2026-07-18T23:00:00Z",
            "startedAt": "2026-07-18T23:30:00Z",
            "version": 1
        }),
    )]);

    fixture.clock.advance(Duration::from_secs(300));
    fixture.store.apply_snapshot(vec![job_fixture(
        "claude-code:s1",
        json!({
            "lifecycle": "ended",
            "outcome": "success",
            // A late snapshot re-states the anchors; the stored ones win.
            "createdAt": "2026-07-19T00:00:00Z",
            "startedAt": "2026-07-19T00:00:00Z",
            "version": 2
        }),
    )]);

    assert!(!has_job(&fixture.store.jobs_json(), "claude-code:s1"));

    let departed = fixture.store.take_departed();
    assert_eq!(departed.len(), 1);
    let gone = wire(&departed[0]);
    assert_eq!(text(&gone, "/id"), "claude-code:s1");
    assert_eq!(text(&gone, "/lifecycle"), "ended");
    assert_eq!(text(&gone, "/createdAt"), "2026-07-18T23:00:00Z");
    assert_eq!(text(&gone, "/startedAt"), "2026-07-18T23:30:00Z");
    // Missing `endedAt` is filled from the injected clock, never wall time.
    assert_eq!(text(&gone, "/endedAt"), "2026-07-19T00:05:00Z");
}

#[test]
fn test_take_departed_drains_the_buffer() {
    let mut fixture = fixture();
    fixture
        .store
        .apply_snapshot(vec![job_fixture("a1", json!({}))]);
    fixture.store.apply_snapshot(vec![job_fixture(
        "a1",
        json!({ "lifecycle": "ended", "version": 2 }),
    )]);

    assert_eq!(fixture.store.take_departed().len(), 1);
    assert!(fixture.store.take_departed().is_empty());
}

#[test]
fn test_snapshot_fills_empty_job_alias_with_machine_alias() {
    let mut fixture = fixture();

    fixture.store.apply_snapshot(vec![
        job_fixture("a1", json!({ "alias": "" })),
        job_fixture("a2", json!({ "alias": "remote-box" })),
    ]);

    let jobs = fixture.store.jobs_json();
    assert_eq!(text(&job_in(&jobs, "a1"), "/alias"), MACHINE_ALIAS);
    assert_eq!(text(&job_in(&jobs, "a2"), "/alias"), "remote-box");
}

#[test]
fn test_hub_publishes_display_actions() {
    // Surfaces consume these actions without rewriting the task.
    let mut fixture = fixture();

    fixture.store.apply_snapshot(vec![job_fixture(
        "a1",
        json!({
            "actions": [],
            "location": { "openURL": "file:///tmp", "focusHint": "Demo · nerve" }
        }),
    )]);

    let stored = job_in(&fixture.store.jobs_json(), "a1");
    assert_eq!(text(&stored, "/actions/0/kind"), "open");
    assert_eq!(text(&stored, "/actions/1/kind"), "copy_summary");
}

// ── 3. Legacy child hygiene (`Subject.swift:127`) ───────────────────────────

#[test]
fn test_legacy_child_with_parent_job_id_is_dropped() {
    let mut fixture = fixture();

    fixture.store.apply_snapshot(vec![job_fixture(
        "child-1",
        json!({ "extensions": { "parentJobId": "claude-code:s1" } }),
    )]);

    assert!(!has_job(&fixture.store.jobs_json(), "child-1"));
}

#[test]
fn test_legacy_child_with_paint_ribbon_false_is_dropped() {
    let mut fixture = fixture();

    fixture.store.apply_snapshot(vec![job_fixture(
        "child-2",
        json!({ "extensions": { "paintRibbon": false } }),
    )]);

    assert!(!has_job(&fixture.store.jobs_json(), "child-2"));
}

#[test]
fn test_legacy_child_with_two_colon_id_is_dropped() {
    let mut fixture = fixture();

    // `{producer}:{session}` carries one `:`; legacy children carry two or more.
    fixture
        .store
        .apply_snapshot(vec![job_fixture("claude-code:s1:agent-7", json!({}))]);

    assert!(!has_job(
        &fixture.store.jobs_json(),
        "claude-code:s1:agent-7"
    ));
}

#[test]
fn test_legacy_child_with_subagent_role_is_dropped() {
    let mut fixture = fixture();

    fixture.store.apply_snapshot(vec![job_fixture(
        "child-4",
        json!({ "extensions": { "role": "subagent" } }),
    )]);

    assert!(!has_job(&fixture.store.jobs_json(), "child-4"));
}

#[test]
fn test_group_role_survives_multi_colon_id() {
    // Explicit `role=group|member|job` is never noise, even with 2+ colons.
    let mut fixture = fixture();

    fixture.store.apply_snapshot(vec![job_fixture(
        "molq:group:1",
        json!({ "extensions": { "role": "group" } }),
    )]);

    assert!(has_job(&fixture.store.jobs_json(), "molq:group:1"));
}

// ── 4. Events: dedupe, ordering, create-on-miss ─────────────────────────────

#[test]
fn test_repeated_event_id_is_ignored() {
    let mut fixture = fixture();
    fixture
        .store
        .apply_snapshot(vec![job_fixture("a1", json!({}))]);

    let patch = json!({ "attention": { "level": "urgent", "reason": "input" } });
    assert!(fixture.store.apply_event(event_fixture(
        "ev1",
        "a1",
        "attention.changed",
        patch.clone()
    )));
    assert!(
        !fixture
            .store
            .apply_event(event_fixture("ev1", "a1", "attention.changed", patch))
    );
}

#[test]
fn test_dedupe_capacity_constants_match_the_swift_store() {
    // Behavioural proof would need 4000+ events per run; the port's contract is
    // the pair of constants at `SubjectStore.swift:26` and `:860`.
    assert_eq!(MAX_SEEN_EVENTS, 4_000);
    assert_eq!(SEEN_EVENT_EVICT_BATCH, 500);
}

#[test]
fn test_event_with_older_version_is_rejected() {
    let mut fixture = fixture();
    fixture
        .store
        .apply_snapshot(vec![job_fixture("a1", json!({ "version": 5 }))]);

    let applied = fixture.store.apply_event(event_fixture(
        "ev2",
        "a1",
        "attention.changed",
        json!({
            "version": 3,
            "attention": { "level": "urgent", "reason": "input" }
        }),
    ));

    assert!(!applied);
    let stored = job_in(&fixture.store.jobs_json(), "a1");
    assert_eq!(text(&stored, "/attention/level"), "none");
    assert_eq!(number(&stored, "/version"), 5);
}

#[test]
fn test_event_creates_job_on_miss_when_name_present() {
    let mut fixture = fixture();

    let applied = fixture.store.apply_event(event_fixture(
        "ev1",
        "claude-code:s9",
        "current.changed",
        json!({
            "name": "nerve",
            "current": { "type": "thinking", "summary": "planning" }
        }),
    ));

    assert!(applied);
    let created = job_in(&fixture.store.jobs_json(), "claude-code:s9");
    assert_eq!(text(&created, "/name"), "nerve");
    assert_eq!(text(&created, "/kind"), "session");
    assert_eq!(text(&created, "/lifecycle"), "active");
    assert_eq!(text(&created, "/current/type"), "thinking");
    assert_eq!(text(&created, "/producer/id"), "t");
    // No `alias` on the event → the injected machine alias.
    assert_eq!(text(&created, "/alias"), MACHINE_ALIAS);
}

#[test]
fn test_event_creates_job_on_miss_for_job_created_kind() {
    let mut fixture = fixture();

    let applied = fixture.store.apply_event(event_fixture(
        "ev1",
        "claude-code:s9",
        "job.created",
        json!({ "alias": "remote-box" }),
    ));

    assert!(applied);
    let created = job_in(&fixture.store.jobs_json(), "claude-code:s9");
    // Nameless create falls back to the job id (`SubjectStore.swift:467`).
    assert_eq!(text(&created, "/name"), "claude-code:s9");
    assert_eq!(text(&created, "/alias"), "remote-box");
}

#[test]
fn test_event_for_unknown_job_without_name_is_ignored() {
    let mut fixture = fixture();

    let applied = fixture.store.apply_event(event_fixture(
        "ev1",
        "claude-code:s9",
        "current.changed",
        json!({ "current": { "type": "thinking" } }),
    ));

    assert!(!applied);
    assert!(array(&fixture.store.jobs_json()).is_empty());
}

// ── 5. Timeline ─────────────────────────────────────────────────────────────

#[test]
fn test_timeline_keeps_only_the_newest_forty_entries() {
    let mut fixture = fixture();
    fixture
        .store
        .apply_snapshot(vec![job_fixture("a1", json!({}))]);

    for step in 0..45 {
        let applied = fixture.store.apply_event(event_fixture(
            &format!("ev{step}"),
            "a1",
            "current.changed",
            json!({ "current": { "type": "thinking", "summary": format!("step-{step}") } }),
        ));
        assert!(applied, "event ev{step} must apply");
    }

    let timeline = timeline_of(&fixture.store.jobs_json(), "a1");
    assert_eq!(timeline.len(), 40);
    // Newest first; `current.changed` titles come from `current.summary`.
    assert_eq!(text(&timeline[0], "/title"), "step-44");
    assert_eq!(text(&timeline[39], "/title"), "step-5");
    assert_eq!(text(&timeline[0], "/kind"), "current.changed");
    assert_eq!(text(&timeline[0], "/jobId"), "a1");
}

#[test]
fn test_heartbeat_events_stay_out_of_the_timeline() {
    let mut fixture = fixture();
    fixture
        .store
        .apply_snapshot(vec![job_fixture("a1", json!({}))]);
    fixture.store.apply_event(event_fixture(
        "ev1",
        "a1",
        "current.changed",
        json!({ "current": { "type": "thinking", "summary": "planning" } }),
    ));

    // A heartbeat still counts as applied — it just leaves no trace.
    assert!(
        fixture
            .store
            .apply_event(event_fixture("ev2", "a1", "heartbeat", json!({})))
    );

    let timeline = timeline_of(&fixture.store.jobs_json(), "a1");
    assert_eq!(timeline.len(), 1);
    assert_eq!(text(&timeline[0], "/kind"), "current.changed");
}

// ── 6. Reaper (acceptance A8) ───────────────────────────────────────────────

#[test]
fn test_reaper_ends_job_when_pid_probe_reports_dead() {
    let mut fixture = fixture();
    fixture.probe.set(4242, PidState::Dead);
    fixture.store.apply_snapshot(vec![job_fixture(
        "claude-code:s1",
        json!({ "extensions": { "pid": 4242 } }),
    )]);

    fixture.clock.advance(Duration::from_secs(10));
    fixture.store.expire_and_reap();

    assert!(!has_job(&fixture.store.jobs_json(), "claude-code:s1"));
    let departed = fixture.store.take_departed();
    assert_eq!(departed.len(), 1);
    let gone = wire(&departed[0]);
    assert_eq!(text(&gone, "/lifecycle"), "ended");
    assert_eq!(text(&gone, "/outcome"), "cancelled");
    assert_eq!(text(&gone, "/extensions/endReason"), "process_gone");
    assert_eq!(text(&gone, "/endedAt"), "2026-07-19T00:00:10Z");
}

#[test]
fn test_reaper_reads_string_pid_extension() {
    let mut fixture = fixture();
    fixture.probe.set(4242, PidState::Dead);
    fixture.store.apply_snapshot(vec![job_fixture(
        "claude-code:s1",
        json!({ "extensions": { "pid": "4242" } }),
    )]);

    fixture.clock.advance(Duration::from_secs(10));
    fixture.store.expire_and_reap();

    assert!(!has_job(&fixture.store.jobs_json(), "claude-code:s1"));
    let departed = fixture.store.take_departed();
    assert_eq!(departed.len(), 1);
    assert_eq!(
        text(&wire(&departed[0]), "/extensions/endReason"),
        "process_gone"
    );
}

#[test]
fn test_reaper_keeps_job_when_pid_is_alive() {
    let mut fixture = fixture();
    fixture.probe.set(4242, PidState::Alive);
    fixture.store.apply_snapshot(vec![job_fixture(
        "claude-code:s1",
        json!({ "extensions": { "pid": 4242 } }),
    )]);

    fixture.clock.advance(Duration::from_secs(10));
    fixture.store.expire_and_reap();

    assert!(has_job(&fixture.store.jobs_json(), "claude-code:s1"));
    assert!(fixture.store.take_departed().is_empty());
}

#[test]
fn test_reaper_treats_denied_probe_as_alive() {
    // EPERM: the process exists, we just cannot signal it.
    let mut fixture = fixture();
    fixture.probe.set(4242, PidState::Denied);
    fixture.store.apply_snapshot(vec![job_fixture(
        "claude-code:s1",
        json!({ "extensions": { "pid": 4242 } }),
    )]);

    fixture.clock.advance(Duration::from_secs(10));
    fixture.store.expire_and_reap();

    assert!(has_job(&fixture.store.jobs_json(), "claude-code:s1"));
    assert!(fixture.store.take_departed().is_empty());
}

#[test]
fn test_reaper_skips_jobs_younger_than_min_age() {
    // Min age 3s: a pid may not be published yet right after spawn.
    let mut fixture = fixture();
    fixture.probe.set(4242, PidState::Dead);
    fixture.store.apply_snapshot(vec![job_fixture(
        "claude-code:s1",
        json!({ "extensions": { "pid": 4242 } }),
    )]);

    fixture.clock.advance(Duration::from_secs(2));
    fixture.store.expire_and_reap();

    assert!(has_job(&fixture.store.jobs_json(), "claude-code:s1"));
}

#[test]
fn test_alias_does_not_change_local_authority() {
    let mut fixture = fixture();
    fixture.probe.set(4242, PidState::Dead);
    fixture.store.apply_snapshot(vec![job_fixture(
        "claude-code:s1",
        json!({
            "alias": "remote-box",
            "extensions": { "pid": 4242 }
        }),
    )]);

    fixture.clock.advance(Duration::from_secs(10));
    fixture.store.expire_and_reap();

    assert!(!has_job(&fixture.store.jobs_json(), "claude-code:s1"));
    assert_eq!(fixture.store.take_departed().len(), 1);
}

#[test]
fn test_reaper_ignores_pid_below_two() {
    // Every pid reads Dead here — pid 0/1 must still never be probed or reaped.
    let mut fixture = fixture_with_default_pid(PidState::Dead);
    fixture.store.apply_snapshot(vec![
        job_fixture("job-pid-one", json!({ "extensions": { "pid": 1 } })),
        job_fixture("job-pid-zero", json!({ "extensions": { "pid": 0 } })),
    ]);

    fixture.clock.advance(Duration::from_secs(10));
    fixture.store.expire_and_reap();

    let jobs = fixture.store.jobs_json();
    assert!(has_job(&jobs, "job-pid-one"));
    assert!(has_job(&jobs, "job-pid-zero"));
}

// ── 7. Pending actions (dormant subsystem) ──────────────────────────────────

/// Job carrying one declared action, so pending has something to bind to.
fn job_with_open_action(id: &str) -> Job {
    job_fixture(
        id,
        json!({
            "actions": [{
                "id": "open",
                "title": "Open",
                "kind": "open",
                "state": "available",
                "destructive": false,
                "confirmationRequired": false
            }]
        }),
    )
}

#[test]
fn test_pending_enqueue_marks_the_declared_action_pending() {
    let mut fixture = fixture();
    fixture
        .store
        .apply_snapshot(vec![job_with_open_action("a1")]);

    assert!(fixture.store.enqueue_pending("p1", "a1", "open"));

    let pending = fixture.store.pending_json(Some("t"));
    assert_eq!(array(&pending).len(), 1);
    let request = &array(&pending)[0];
    assert_eq!(text(request, "/id"), "p1");
    assert_eq!(text(request, "/jobId"), "a1");
    assert_eq!(text(request, "/producerId"), "t");
    assert_eq!(text(request, "/actionId"), "open");
    assert_eq!(text(request, "/state"), "pending");
    assert_eq!(text(request, "/requestedAt"), T0);
    // TTL 3600s from the injected clock.
    assert_eq!(text(request, "/expiresAt"), "2026-07-19T01:00:00Z");

    let stored = job_in(&fixture.store.jobs_json(), "a1");
    assert_eq!(text(&stored, "/actions/0/state"), "pending");
}

#[test]
fn test_pending_queue_keeps_only_the_newest_two_hundred() {
    let mut fixture = fixture();
    fixture
        .store
        .apply_snapshot(vec![job_with_open_action("a1")]);

    for n in 0..205 {
        assert!(
            fixture
                .store
                .enqueue_pending(&format!("p{n}"), "a1", "open")
        );
    }

    let pending = fixture.store.pending_json(None);
    let list = array(&pending);
    assert_eq!(list.len(), 200);
    // Newest first; the five oldest fell off.
    assert_eq!(text(&list[0], "/id"), "p204");
    assert!(
        !list
            .iter()
            .any(|request| request.get("id").and_then(Value::as_str) == Some("p0"))
    );
}

#[test]
fn test_pending_stays_open_before_the_ttl() {
    let mut fixture = fixture();
    fixture
        .store
        .apply_snapshot(vec![job_with_open_action("a1")]);
    fixture.store.enqueue_pending("p1", "a1", "open");

    fixture.clock.advance(Duration::from_secs(3_599));
    fixture.store.expire_and_reap();

    assert_eq!(array(&fixture.store.pending_json(None)).len(), 1);
    let stored = job_in(&fixture.store.jobs_json(), "a1");
    assert_eq!(text(&stored, "/actions/0/state"), "pending");
}

#[test]
fn test_pending_expires_after_the_ttl() {
    let mut fixture = fixture();
    fixture
        .store
        .apply_snapshot(vec![job_with_open_action("a1")]);
    fixture.store.enqueue_pending("p1", "a1", "open");

    fixture.clock.advance(Duration::from_secs(3_601));
    fixture.store.expire_and_reap();

    // `pending_json` lists open requests only.
    assert!(array(&fixture.store.pending_json(None)).is_empty());
    let stored = job_in(&fixture.store.jobs_json(), "a1");
    assert_eq!(text(&stored, "/actions/0/state"), "expired");
}

#[test]
fn test_complete_pending_returns_false_for_unknown_id() {
    let mut fixture = fixture();
    fixture
        .store
        .apply_snapshot(vec![job_with_open_action("a1")]);

    assert!(
        !fixture
            .store
            .complete_pending("nope", ActionState::Succeeded, Some("x"), None)
    );
}

#[test]
fn test_complete_pending_returns_false_on_producer_mismatch() {
    let mut fixture = fixture();
    fixture
        .store
        .apply_snapshot(vec![job_with_open_action("a1")]);
    fixture.store.enqueue_pending("p1", "a1", "open");

    let completed =
        fixture
            .store
            .complete_pending("p1", ActionState::Succeeded, Some("x"), Some("other"));

    assert!(!completed);
    let stored = job_in(&fixture.store.jobs_json(), "a1");
    assert_eq!(text(&stored, "/actions/0/state"), "pending");
}

#[test]
fn test_complete_pending_resets_the_action_to_available() {
    let mut fixture = fixture();
    fixture
        .store
        .apply_snapshot(vec![job_with_open_action("a1")]);
    fixture.store.enqueue_pending("p1", "a1", "open");

    let completed =
        fixture
            .store
            .complete_pending("p1", ActionState::Succeeded, Some("done"), Some("t"));

    assert!(completed);
    // The request is no longer open …
    assert!(array(&fixture.store.pending_json(None)).is_empty());
    // … and the action is re-armed rather than frozen on `succeeded`.
    let stored = job_in(&fixture.store.jobs_json(), "a1");
    assert_eq!(text(&stored, "/actions/0/state"), "available");
}

// ── 8. Clear ────────────────────────────────────────────────────────────────

#[test]
fn test_clear_empties_jobs_without_emitting_departed() {
    let mut fixture = fixture();
    fixture
        .store
        .apply_snapshot(vec![job_fixture("a1", json!({}))]);

    fixture.store.clear();

    assert!(array(&fixture.store.jobs_json()).is_empty());
    // `clear` is an admin wipe, not a lifecycle transition.
    assert!(fixture.store.take_departed().is_empty());
}

#[test]
fn test_clear_forgets_seen_event_ids() {
    let mut fixture = fixture();
    fixture
        .store
        .apply_snapshot(vec![job_fixture("a1", json!({}))]);
    let patch = json!({ "attention": { "level": "urgent", "reason": "input" } });
    assert!(fixture.store.apply_event(event_fixture(
        "ev1",
        "a1",
        "attention.changed",
        patch.clone()
    )));

    fixture.store.clear();
    fixture
        .store
        .apply_snapshot(vec![job_fixture("a1", json!({}))]);

    assert!(
        fixture
            .store
            .apply_event(event_fixture("ev1", "a1", "attention.changed", patch))
    );
}
