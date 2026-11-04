//! `instance.rs` — one surface per tmux server.
//!
//! ─────────────────────────────────────────────────────────────────────────
//! Tests are never edited to fit an implementation.
//! ─────────────────────────────────────────────────────────────────────────
//!
//! ```ignore
//! // nerve_surface_core::instance
//!
//! /// The tmux user option that names the surface process.
//! pub const SURFACE_PID_KEY: &str = "@nerve_surface_pid";
//!
//! /// `kill(pid, 0)` — is that process still there?
//! pub trait PidProbe { fn is_alive(&self, pid: i32) -> bool; }
//!
//! #[derive(Clone, Copy, Debug, PartialEq, Eq)]
//! pub enum Claim {
//!     /// This process owns the surface; the option now names our pid.
//!     Owned,
//!     /// A live surface already holds it: exit 0 having opened nothing.
//!     Yield { pid: i32 },
//! }
//!
//! impl Claim {
//!     /// Whether the caller may open the SSE connection. Only `Owned` may —
//!     /// a second surface must never become a second refcount holder.
//!     pub fn may_stream(self) -> bool;
//! }
//!
//! pub struct InstanceGuard<O: tmux::OptionStore, P: PidProbe> { /* … */ }
//!
//! impl<O: tmux::OptionStore, P: PidProbe> InstanceGuard<O, P> {
//!     /// `me` is this process's pid, injected rather than read, so the guard
//!     /// is a unit test rather than a process.
//!     pub fn new(options: O, probe: P, me: i32) -> Self;
//!
//!     /// Read `@nerve_surface_pid`, and:
//!     ///   * unset / blank / unparsable / `<= 1`  -> claim it, `Owned`
//!     ///   * our own pid                          -> claim it, `Owned`
//!     ///   * another pid, alive                   -> `Yield { pid }`, no write
//!     ///   * another pid, dead                    -> claim it, `Owned`
//!     pub fn claim(&mut self) -> Result<Claim, S::Error>;
//! }
//! ```
//!
//! The lock lives in a tmux option, not in a file: the surface keeps **no**
//! state on disk, and an option dies with the server it
//! belongs to, which is exactly the lock's intended lifetime.
//!
//! `pid <= 1` is never an owner — 0 is "no process" and 1 is init — the same
//! rule the hub applies to producer pids (`crates/nerve-hub/src/model/job.rs:136`).
//!
//! Determinism: fake option store, fake pid probe, injected pid. No process is
//! signalled, no tmux is started, no clock, no socket, no filesystem.

use std::cell::RefCell;
use std::rc::Rc;

use nerve_surface_core::instance::{Claim, InstanceGuard, LockStore, PidProbe, SURFACE_PID_KEY};

/// The store's failure, standing in for whatever a real surface's is — a tmux
/// error, a registry error. The guard only has to propagate it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StoreGone;

const ME: i32 = 4242;
const OTHER: i32 = 999;

/// Every option read and write the guard performed, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Op {
    Get(String),
    Set(String, String),
}

type Ops = Rc<RefCell<Vec<Op>>>;
type Probed = Rc<RefCell<Vec<i32>>>;

// ── Fakes ───────────────────────────────────────────────────────────────────

struct FakeOptions {
    stored: Option<String>,
    read_error: bool,
    ops: Ops,
}

impl LockStore for FakeOptions {
    type Error = StoreGone;

    fn get(&mut self, name: &str) -> Result<Option<String>, Self::Error> {
        self.ops.borrow_mut().push(Op::Get(name.to_string()));
        if self.read_error {
            return Err(StoreGone);
        }
        Ok(self.stored.clone())
    }

    fn set(&mut self, name: &str, value: &str) -> Result<(), Self::Error> {
        self.ops
            .borrow_mut()
            .push(Op::Set(name.to_string(), value.to_string()));
        self.stored = Some(value.to_string());
        Ok(())
    }
}

struct FakePids {
    alive: Vec<i32>,
    probed: Probed,
}

impl PidProbe for FakePids {
    fn is_alive(&self, pid: i32) -> bool {
        self.probed.borrow_mut().push(pid);
        self.alive.contains(&pid)
    }
}

// ── Harness ─────────────────────────────────────────────────────────────────

struct Wires {
    ops: Ops,
    probed: Probed,
}

impl Wires {
    fn ops(&self) -> Vec<Op> {
        self.ops.borrow().clone()
    }

    fn probed(&self) -> Vec<i32> {
        self.probed.borrow().clone()
    }
}

fn guard(stored: Option<&str>, alive: &[i32]) -> (InstanceGuard<FakeOptions, FakePids>, Wires) {
    guard_with(stored, alive, false)
}

fn guard_with(
    stored: Option<&str>,
    alive: &[i32],
    read_error: bool,
) -> (InstanceGuard<FakeOptions, FakePids>, Wires) {
    let ops: Ops = Rc::new(RefCell::new(Vec::new()));
    let probed: Probed = Rc::new(RefCell::new(Vec::new()));

    let options = FakeOptions {
        stored: stored.map(str::to_string),
        read_error,
        ops: Rc::clone(&ops),
    };
    let pids = FakePids {
        alive: alive.to_vec(),
        probed: Rc::clone(&probed),
    };

    (InstanceGuard::new(options, pids, ME), Wires { ops, probed })
}

fn claimed(guard: &mut InstanceGuard<FakeOptions, FakePids>) -> Claim {
    guard.claim().expect("claiming must not fail")
}

// ── Taking the lock ─────────────────────────────────────────────────────────

/// Acceptance A6: on start the option is claimed.
#[test]
fn test_an_unclaimed_surface_is_taken_over() {
    let (mut guard, wires) = guard(None, &[]);

    assert_eq!(claimed(&mut guard), Claim::Owned);
    assert_eq!(
        wires.ops(),
        vec![
            Op::Get(SURFACE_PID_KEY.to_string()),
            Op::Set(SURFACE_PID_KEY.to_string(), ME.to_string()),
        ]
    );
}

#[test]
fn test_an_unparsable_option_counts_as_unclaimed() {
    let (mut guard, wires) = guard(Some("not-a-pid"), &[]);

    assert_eq!(claimed(&mut guard), Claim::Owned);
    assert!(
        wires.probed().is_empty(),
        "nothing that is not a pid may be signalled"
    );
}

/// 0 is "no process" and 1 is init: neither can be a surface.
#[test]
fn test_pid_zero_and_pid_one_are_never_owners() {
    for stored in ["0", "1", "-5"] {
        let (mut guard, wires) = guard(Some(stored), &[0, 1]);

        assert_eq!(claimed(&mut guard), Claim::Owned, "stored `{stored}`");
        assert!(wires.probed().is_empty(), "stored `{stored}`");
    }
}

// ── Yielding to a live surface ──────────────────────────────────────────────

/// Acceptance A6: a second `run` that sees a live pid exits at once.
#[test]
fn test_a_live_owner_is_yielded_to() {
    let (mut guard, wires) = guard(Some("999"), &[OTHER]);

    assert_eq!(claimed(&mut guard), Claim::Yield { pid: OTHER });
    assert_eq!(wires.ops(), vec![Op::Get(SURFACE_PID_KEY.to_string())]);
}

/// …and "exits at once" means it never becomes a second refcount holder.
#[test]
fn test_only_an_owned_claim_may_open_a_stream() {
    assert!(Claim::Owned.may_stream());
    assert!(!Claim::Yield { pid: OTHER }.may_stream());
}

// ── Taking over ─────────────────────────────────────────────────────────────

/// Acceptance A6: the recorded pid is dead, so this process takes over and
/// rewrites the option.
#[test]
fn test_a_dead_owner_is_taken_over() {
    let (mut guard, wires) = guard(Some("999"), &[]);

    assert_eq!(claimed(&mut guard), Claim::Owned);
    assert_eq!(
        wires.ops(),
        vec![
            Op::Get(SURFACE_PID_KEY.to_string()),
            Op::Set(SURFACE_PID_KEY.to_string(), ME.to_string()),
        ]
    );
    assert_eq!(wires.probed(), vec![OTHER]);
}

/// Our own pid is not somebody else's: a re-claim by the running surface must
/// not make it stand down against itself.
#[test]
fn test_our_own_pid_is_reclaimed_rather_than_yielded_to() {
    let (mut guard, _wires) = guard(Some("4242"), &[ME]);

    assert_eq!(claimed(&mut guard), Claim::Owned);
}

// ── tmux going away ─────────────────────────────────────────────────────────

#[test]
fn test_a_tmux_failure_while_reading_is_reported() {
    let (mut guard, _wires) = guard_with(None, &[], true);

    let result = guard.claim();

    assert!(matches!(result, Err(StoreGone)));
}
