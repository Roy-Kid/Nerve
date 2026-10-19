//! `stream.rs` — one SSE attach, and how long to wait before the next one
//! (spec T5 · acceptance A4).
//!
//! ─────────────────────────────────────────────────────────────────────────
//! API CONTRACT — the implementer fills `src/stream.rs` to satisfy this file.
//! Tests are never edited to fit an implementation.
//! ─────────────────────────────────────────────────────────────────────────
//!
//! ```ignore
//! // nerve_tmux_surface::stream
//!
//! /// Doubling reconnect delay: 0.5s, capped at 30s, reset by any attach that
//! /// delivered a frame.
//! #[derive(Clone, Copy, Debug)]
//! pub struct Backoff { /* … */ }
//!
//! impl Backoff {
//!     pub const FIRST: std::time::Duration = std::time::Duration::from_millis(500);
//!     pub const CAP: std::time::Duration = std::time::Duration::from_secs(30);
//!
//!     pub fn new() -> Self;              // == Default
//!     /// The delay to wait after this failure; doubles for the next one.
//!     pub fn fail(&mut self) -> std::time::Duration;
//!     /// What the next `fail()` will return, without consuming it.
//!     pub fn peek(&self) -> std::time::Duration;
//!     /// Back to `FIRST`.
//!     pub fn reset(&mut self);
//! }
//!
//! /// One long-lived `GET /v1/stream?surface=tmux`. Blocking and pull-based:
//! /// this crate has no async runtime (see `Cargo.toml`).
//! pub trait FrameSource {
//!     /// Open the connection. This is the surface's refcount presence
//!     /// (spec Summary): holding it open is what keeps the hub alive.
//!     fn open(&mut self) -> Result<(), StreamError>;
//!     /// Block for the next frame. `Ok(None)` = the hub closed the stream.
//!     fn next_frame(&mut self) -> Result<Option<frame::Frame>, StreamError>;
//! }
//!
//! #[derive(Debug)]
//! pub enum StreamError {
//!     /// Nothing answered on 127.0.0.1:17890.
//!     Unreachable,
//!     /// The connection died, or carried something that would not decode.
//!     Broken(String),
//! }
//!
//! pub struct StreamSession<S: FrameSource, W: tmux::SegmentWriter> { /* … */ }
//!
//! impl<S: FrameSource, W: tmux::SegmentWriter> StreamSession<S, W> {
//!     pub fn new(
//!         source: S,
//!         writer: W,
//!         summary: summary::SummaryRenderer,
//!         offline: impl Into<String>,
//!     ) -> Self;
//!
//!     /// One attach, start to finish. Never sleeps — `Backoff` computes the
//!     /// delay and the composition root waits, so this module carries no
//!     /// time and no runtime.
//!     pub fn attach(&mut self) -> Attempt;
//! }
//!
//! #[derive(Clone, Copy, Debug, PartialEq, Eq)]
//! pub enum Attempt {
//!     /// The attach is over after delivering `frames` frames. Zero means the
//!     /// hub was never reachable.
//!     Ended { frames: usize },
//!     /// tmux itself is gone: the caller exits 0 and drops the connection,
//!     /// which is how the hub learns this surface left.
//!     ServerGone,
//! }
//! ```
//!
//! ─────────────────────────────────────────────────────────────────────────
//! WHEN THE OFFLINE PLACEHOLDER IS WRITTEN
//! ─────────────────────────────────────────────────────────────────────────
//!
//! Every attach that ends writes it — a failed `open`, a clean close, and a
//! connection that died mid-flight alike. In all three there is no longer a
//! hub behind the numbers, and a status line that keeps painting stale counts
//! lies. `Attempt::ServerGone` writes nothing: there is no tmux left to write
//! to.
//!
//! A tmux error that is *not* `ServerGone` is swallowed and the session
//! continues (fail-open, CLAUDE.md invariant 2).
//!
//! Determinism: scripted fake source, fake writer, literal frames, hard-coded
//! segment goldens. No socket, no clock, no sleeping, no hub.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use nerve_surface_core::frame::Frame;
use nerve_tmux_surface::stream::{Attempt, Backoff, FrameSource, StreamError, StreamSession};
use nerve_tmux_surface::summary::SummaryRenderer;
use nerve_tmux_surface::tmux::{SegmentWriter, TmuxError};

/// One open job: `Tally { running: 1 }` under the default template.
const ONE_RUNNING: &str = r#"{"jobs":[{"id":"a","lifecycle":"active"}],"departed":[]}"#;
const ONE_RUNNING_SEGMENT: &str = "#[fg=blue]1>#[default]";

/// One open job and one failed one.
const RUNNING_AND_PROBLEM: &str = r#"{"jobs":[
    {"id":"a","lifecycle":"active"},
    {"id":"b","lifecycle":"active","outcome":"failure"}
],"departed":[]}"#;
const RUNNING_AND_PROBLEM_SEGMENT: &str = "#[fg=red]1!#[default] #[fg=blue]1>#[default]";

// ── Fakes ───────────────────────────────────────────────────────────────────

/// What the fake connection does next.
enum Step {
    /// Deliver this frame.
    Frame(&'static str),
    /// The hub closed the stream.
    Close,
    /// The connection died.
    Broken,
}

struct FakeSource {
    /// Whether each successive `open()` succeeds.
    opens: VecDeque<bool>,
    steps: VecDeque<Step>,
    reads: Rc<Cell<usize>>,
}

impl FrameSource for FakeSource {
    fn open(&mut self) -> Result<(), StreamError> {
        match self.opens.pop_front() {
            Some(true) | None => Ok(()),
            Some(false) => Err(StreamError::Unreachable),
        }
    }

    fn next_frame(&mut self) -> Result<Option<Frame>, StreamError> {
        self.reads.set(self.reads.get() + 1);
        match self.steps.pop_front() {
            Some(Step::Frame(raw)) => {
                let frame = nerve_surface_core::testkit::frame(raw);
                Ok(Some(frame))
            }
            Some(Step::Close) | None => Ok(None),
            Some(Step::Broken) => Err(StreamError::Broken("connection reset".to_string())),
        }
    }
}

struct FakeWriter {
    log: Rc<RefCell<Vec<String>>>,
    errors: VecDeque<TmuxError>,
}

impl SegmentWriter for FakeWriter {
    fn write_segment(&mut self, text: &str) -> Result<(), TmuxError> {
        self.log.borrow_mut().push(text.to_string());
        match self.errors.pop_front() {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }
}

// ── Harness ─────────────────────────────────────────────────────────────────

struct Wires {
    log: Rc<RefCell<Vec<String>>>,
    reads: Rc<Cell<usize>>,
}

impl Wires {
    fn written(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
}

fn session(
    opens: Vec<bool>,
    steps: Vec<Step>,
    errors: Vec<TmuxError>,
) -> (StreamSession<FakeSource, FakeWriter>, Wires) {
    session_with(opens, steps, errors, SummaryRenderer::DEFAULT_OFFLINE)
}

fn session_with(
    opens: Vec<bool>,
    steps: Vec<Step>,
    errors: Vec<TmuxError>,
    offline: &str,
) -> (StreamSession<FakeSource, FakeWriter>, Wires) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let reads = Rc::new(Cell::new(0));

    let source = FakeSource {
        opens: opens.into(),
        steps: steps.into(),
        reads: Rc::clone(&reads),
    };
    let writer = FakeWriter {
        log: Rc::clone(&log),
        errors: errors.into(),
    };

    (
        StreamSession::new(source, writer, SummaryRenderer::default(), offline),
        Wires { log, reads },
    )
}

// ════════════════════════════════════════════════════════════════════════════
// Backoff (acceptance A4)
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_a_fresh_backoff_starts_at_half_a_second() {
    assert_eq!(Backoff::new().peek(), Duration::from_millis(500));
}

#[test]
fn test_the_first_delay_and_the_cap_are_the_documented_constants() {
    assert_eq!(Backoff::FIRST, Duration::from_millis(500));
    assert_eq!(Backoff::CAP, Duration::from_secs(30));
}

/// Acceptance A4, hard-coded: `0.5, 1, 2, 4, 8, 16, 30, 30`.
#[test]
fn test_consecutive_failures_double_up_to_a_thirty_second_cap() {
    let mut backoff = Backoff::new();

    let delays: Vec<Duration> = (0..8).map(|_| backoff.fail()).collect();

    assert_eq!(
        delays,
        vec![
            Duration::from_millis(500),
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(4),
            Duration::from_secs(8),
            Duration::from_secs(16),
            Duration::from_secs(30),
            Duration::from_secs(30),
        ]
    );
}

#[test]
fn test_the_cap_holds_however_long_the_hub_stays_away() {
    let mut backoff = Backoff::new();
    for _ in 0..100 {
        backoff.fail();
    }

    assert_eq!(backoff.fail(), Duration::from_secs(30));
}

/// Acceptance A4: one success and the next outage starts over at 0.5s.
#[test]
fn test_a_success_resets_the_delay_to_the_first_step() {
    let mut backoff = Backoff::new();
    for _ in 0..5 {
        backoff.fail();
    }

    backoff.reset();

    assert_eq!(backoff.fail(), Duration::from_millis(500));
}

#[test]
fn test_peek_reports_what_the_next_failure_will_cost() {
    let mut backoff = Backoff::new();
    backoff.fail();

    assert_eq!(backoff.peek(), Duration::from_secs(1));
    assert_eq!(backoff.fail(), Duration::from_secs(1));
}

#[test]
fn test_resetting_a_fresh_backoff_changes_nothing() {
    let mut backoff = Backoff::new();
    backoff.reset();

    assert_eq!(backoff.fail(), Duration::from_millis(500));
}

// ════════════════════════════════════════════════════════════════════════════
// StreamSession (acceptance A4)
// ════════════════════════════════════════════════════════════════════════════

/// Acceptance A4: the first failure writes `@nerve_status_offline`.
#[test]
fn test_an_unreachable_hub_writes_the_offline_placeholder() {
    let (mut session, wires) = session(vec![false], vec![], vec![]);

    let attempt = session.attach();

    assert_eq!(attempt, Attempt::Ended { frames: 0 });
    assert_eq!(wires.written(), vec![SummaryRenderer::DEFAULT_OFFLINE]);
}

#[test]
fn test_an_unreachable_hub_is_never_read_from() {
    let (mut session, wires) = session(vec![false], vec![], vec![]);

    session.attach();

    assert_eq!(wires.reads.get(), 0);
}

#[test]
fn test_the_offline_text_is_the_one_the_session_was_given() {
    let (mut session, wires) = session_with(vec![false], vec![], vec![], "nerve: offline");

    session.attach();

    assert_eq!(wires.written(), vec!["nerve: offline"]);
}

#[test]
fn test_every_frame_is_rendered_into_the_segment() {
    let (mut session, wires) = session(
        vec![true],
        vec![
            Step::Frame(ONE_RUNNING),
            Step::Frame(RUNNING_AND_PROBLEM),
            Step::Close,
        ],
        vec![],
    );

    let attempt = session.attach();

    assert_eq!(attempt, Attempt::Ended { frames: 2 });
    assert_eq!(
        wires.written(),
        vec![
            ONE_RUNNING_SEGMENT,
            RUNNING_AND_PROBLEM_SEGMENT,
            SummaryRenderer::DEFAULT_OFFLINE,
        ]
    );
}

/// A hub that closes the stream is a hub that is going away: the counts must
/// not stay on screen pretending otherwise.
#[test]
fn test_a_closed_stream_ends_the_attach_and_goes_offline() {
    let (mut session, wires) = session(
        vec![true],
        vec![Step::Frame(ONE_RUNNING), Step::Close],
        vec![],
    );

    let attempt = session.attach();

    assert_eq!(attempt, Attempt::Ended { frames: 1 });
    assert_eq!(
        wires.written(),
        vec![ONE_RUNNING_SEGMENT, SummaryRenderer::DEFAULT_OFFLINE]
    );
}

#[test]
fn test_a_connection_that_dies_mid_flight_ends_the_attach_and_goes_offline() {
    let (mut session, wires) = session(
        vec![true],
        vec![Step::Frame(ONE_RUNNING), Step::Broken],
        vec![],
    );

    let attempt = session.attach();

    assert_eq!(attempt, Attempt::Ended { frames: 1 });
    assert_eq!(
        wires.written(),
        vec![ONE_RUNNING_SEGMENT, SummaryRenderer::DEFAULT_OFFLINE]
    );
}

/// Acceptance A4, last clause: after a reconnect the next frame overwrites the
/// placeholder with a live segment.
#[test]
fn test_a_reconnect_overwrites_the_offline_placeholder() {
    let (mut session, wires) = session(
        vec![false, true],
        vec![Step::Frame(ONE_RUNNING), Step::Close],
        vec![],
    );

    let first = session.attach();
    let second = session.attach();

    assert_eq!(first, Attempt::Ended { frames: 0 });
    assert_eq!(second, Attempt::Ended { frames: 1 });
    assert_eq!(
        wires.written(),
        vec![
            SummaryRenderer::DEFAULT_OFFLINE,
            ONE_RUNNING_SEGMENT,
            SummaryRenderer::DEFAULT_OFFLINE,
        ]
    );
}

// ── tmux going away ─────────────────────────────────────────────────────────

/// Acceptance A5: `no server running` stops the session. Nothing more is read
/// (the connection is dropped, which is how the hub's refcount falls).
#[test]
fn test_a_gone_tmux_server_stops_the_session_at_once() {
    let (mut session, wires) = session(
        vec![true],
        vec![
            Step::Frame(ONE_RUNNING),
            Step::Frame(RUNNING_AND_PROBLEM),
            Step::Close,
        ],
        vec![TmuxError::ServerGone],
    );

    let attempt = session.attach();

    assert_eq!(attempt, Attempt::ServerGone);
    assert_eq!(wires.reads.get(), 1);
}

/// …and it does not then try to paint an offline placeholder into a tmux that
/// is not there.
#[test]
fn test_a_gone_tmux_server_writes_no_offline_placeholder() {
    let (mut session, wires) = session(
        vec![true],
        vec![Step::Frame(ONE_RUNNING), Step::Close],
        vec![TmuxError::ServerGone],
    );

    session.attach();

    assert_eq!(wires.written(), vec![ONE_RUNNING_SEGMENT]);
}

/// Fail-open: a tmux hiccup that is not `ServerGone` costs one repaint, not
/// the session (CLAUDE.md invariant 2).
#[test]
fn test_a_transient_tmux_failure_does_not_end_the_session() {
    let (mut session, wires) = session(
        vec![true],
        vec![
            Step::Frame(ONE_RUNNING),
            Step::Frame(RUNNING_AND_PROBLEM),
            Step::Close,
        ],
        vec![TmuxError::Unavailable(
            "resource temporarily unavailable".to_string(),
        )],
    );

    let attempt = session.attach();

    assert_eq!(attempt, Attempt::Ended { frames: 2 });
    assert_eq!(
        wires.written(),
        vec![
            ONE_RUNNING_SEGMENT,
            RUNNING_AND_PROBLEM_SEGMENT,
            SummaryRenderer::DEFAULT_OFFLINE,
        ]
    );
}
