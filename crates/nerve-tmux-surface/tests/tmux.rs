//! `tmux.rs` — the only place this crate talks to tmux (spec T6 ·
//! acceptance A5).
//!
//! ─────────────────────────────────────────────────────────────────────────
//! API CONTRACT — the implementer fills `src/tmux.rs` to satisfy this file.
//! Tests are never edited to fit an implementation.
//! ─────────────────────────────────────────────────────────────────────────
//!
//! ```ignore
//! // nerve_tmux_surface::tmux
//!
//! /// The user option `status-right` interpolates as `#{@nerve_status}`.
//! pub const STATUS_OPTION: &str = "@nerve_status";
//!
//! /// One `tmux` invocation. **Arg array only.** There is no shell anywhere
//! /// in this crate — the same rule `plugins/nerve/hooks/nerve_hook.py`
//! /// follows (spec Domain basis, "tmux(1) 事实").
//! pub trait TmuxRunner {
//!     /// Run `tmux <args>`, answering its trimmed stdout.
//!     fn run(&mut self, args: &[&str]) -> Result<String, TmuxError>;
//! }
//!
//! #[derive(Debug)]
//! pub enum TmuxError {
//!     /// `no server running on …` — the tmux this surface painted is gone.
//!     /// Not a failure: the surface's work is simply over (exit 0).
//!     ServerGone,
//!     /// tmux could not be executed at all (missing binary, spawn refused).
//!     Unavailable(String),
//!     /// tmux ran and refused the command.
//!     Refused { status: i32, stderr: String },
//! }
//!
//! impl TmuxError {
//!     /// Classify a finished invocation from its exit status and stderr.
//!     pub fn from_exit(status: i32, stderr: &str) -> Self;
//!     pub fn is_server_gone(&self) -> bool;
//! }
//!
//! /// Where a rendered segment goes (`stream.rs` writes through this).
//! pub trait SegmentWriter {
//!     fn write_segment(&mut self, text: &str) -> Result<(), TmuxError>;
//! }
//!
//! /// Global tmux user options — this surface's only persistent state, and
//! /// it lives in tmux, not on disk (acceptance A11).
//! pub trait LockStore {   // nerve_surface_core::instance
//!     fn get(&mut self, name: &str) -> Result<Option<String>, TmuxError>;
//!     fn set(&mut self, name: &str, value: &str) -> Result<(), TmuxError>;
//! }
//!
//! pub struct TmuxWriter<R: TmuxRunner> { /* runner */ }
//! impl<R: TmuxRunner> TmuxWriter<R> { pub fn new(runner: R) -> Self; }
//! impl<R: TmuxRunner> SegmentWriter for TmuxWriter<R> { /* set + refresh */ }
//! impl<R: TmuxRunner> LockStore for TmuxWriter<R> { /* show-options / set */ }
//! ```
//!
//! Writing the option is not enough: tmux only repaints the status line after
//! `refresh-client -S` (spec Domain basis). Reading uses `show-options -gqv`,
//! which prints the bare value and stays quiet about options nobody set.
//!
//! Determinism: fake runner, literal argv assertions. No tmux process is
//! started, no clock, no socket, no filesystem.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use nerve_surface_core::instance::LockStore;
use nerve_tmux_surface::tmux::{SegmentWriter, TmuxError, TmuxRunner, TmuxWriter};

/// The segment a live frame produces — spaces and style markers included.
const SEGMENT: &str = "#[fg=red]1!#[default] #[fg=blue]2>#[default]";

/// Every tmux subcommand this crate is allowed to invoke.
const ALLOWED_SUBCOMMANDS: [&str; 4] = ["set", "refresh-client", "show-options", "display-popup"];

type Calls = Rc<RefCell<Vec<Vec<String>>>>;

// ── Fake ────────────────────────────────────────────────────────────────────

struct FakeRunner {
    calls: Calls,
    replies: VecDeque<Result<String, TmuxError>>,
}

impl TmuxRunner for FakeRunner {
    fn run(&mut self, args: &[&str]) -> Result<String, TmuxError> {
        self.calls
            .borrow_mut()
            .push(args.iter().map(|arg| (*arg).to_string()).collect());
        self.replies
            .pop_front()
            .unwrap_or_else(|| Ok(String::new()))
    }
}

fn writer(replies: Vec<Result<String, TmuxError>>) -> (TmuxWriter<FakeRunner>, Calls) {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let runner = FakeRunner {
        calls: Rc::clone(&calls),
        replies: replies.into(),
    };
    (TmuxWriter::new(runner), calls)
}

/// Every recorded invocation, as owned strings.
fn argv(calls: &Calls) -> Vec<Vec<String>> {
    calls.borrow().clone()
}

/// The structural rule acceptance A5 is really about: nothing this crate hands
/// tmux is a shell line.
fn assert_no_shell(calls: &Calls) {
    for call in argv(calls) {
        assert!(!call.is_empty(), "an empty invocation is not a command");
        assert!(
            ALLOWED_SUBCOMMANDS.contains(&call[0].as_str()),
            "unexpected tmux subcommand `{}`",
            call[0]
        );
        assert!(
            !call[0].contains(char::is_whitespace),
            "subcommand `{}` is a packed command line",
            call[0]
        );
        assert!(
            call.len() > 1 || !call[0].contains(' '),
            "single-argument invocation `{}` looks like a shell string",
            call[0]
        );
    }
}

// ── Writing the segment ─────────────────────────────────────────────────────

/// Acceptance A5, hard-coded: exactly `set -g @nerve_status <text>` followed
/// by `refresh-client -S`.
#[test]
fn test_writing_a_segment_sets_the_option_then_refreshes() {
    let (mut writer, calls) = writer(vec![]);

    writer.write_segment(SEGMENT).expect("write must succeed");

    assert_eq!(
        argv(&calls),
        vec![
            vec!["set", "-g", "@nerve_status", SEGMENT],
            vec!["refresh-client", "-S"],
        ]
    );
}

#[test]
fn test_no_invocation_is_ever_a_shell_line() {
    let (mut writer, calls) = writer(vec![]);
    writer.write_segment(SEGMENT).expect("write must succeed");

    assert_no_shell(&calls);
}

/// If the option could not be set there is nothing to refresh.
#[test]
fn test_a_refused_set_reports_and_skips_the_refresh() {
    let (mut writer, calls) = writer(vec![Err(TmuxError::Refused {
        status: 1,
        stderr: "unknown option".to_string(),
    })]);

    let result = writer.write_segment(SEGMENT);

    assert!(matches!(result, Err(TmuxError::Refused { status: 1, .. })));
    assert_eq!(argv(&calls).len(), 1);
}

#[test]
fn test_a_gone_server_during_the_refresh_propagates_too() {
    let (mut writer, calls) = writer(vec![Ok(String::new()), Err(TmuxError::ServerGone)]);

    let result = writer.write_segment(SEGMENT);

    assert!(matches!(result, Err(TmuxError::ServerGone)));
    assert_eq!(argv(&calls).len(), 2);
}

// ── Classifying tmux's answer ───────────────────────────────────────────────

/// What tmux(1) prints when the socket is gone. Not an error the surface
/// reports — it is the surface's cue to exit 0 (acceptance A5).
#[test]
fn test_no_server_running_is_recognised_as_server_gone() {
    let err = TmuxError::from_exit(1, "no server running on /private/tmp/tmux-501/default");

    assert!(matches!(err, TmuxError::ServerGone));
    assert!(err.is_server_gone());
}

#[test]
fn test_any_other_failure_stays_a_refusal() {
    let err = TmuxError::from_exit(1, "unknown command: bogus");

    assert!(!err.is_server_gone());
    assert!(matches!(
        err,
        TmuxError::Refused { status: 1, ref stderr } if stderr == "unknown command: bogus"
    ));
}

// ── Reading and writing user options ────────────────────────────────────────

#[test]
fn test_reading_an_option_uses_show_options_and_returns_the_value() {
    let (mut writer, calls) = writer(vec![Ok("{running}/{total}".to_string())]);

    let value = writer
        .get("@nerve_status_format")
        .expect("read must succeed");

    assert_eq!(value.as_deref(), Some("{running}/{total}"));
    assert_eq!(
        argv(&calls),
        vec![vec!["show-options", "-gqv", "@nerve_status_format"]]
    );
}

#[test]
fn test_an_unset_option_reads_as_absent() {
    let (mut writer, _calls) = writer(vec![Ok(String::new())]);

    assert_eq!(writer.get("@nerve_popup_key").expect("read"), None);
}

#[test]
fn test_writing_an_option_uses_set_g_and_does_not_refresh() {
    let (mut writer, calls) = writer(vec![]);

    writer.set("@nerve_surface_pid", "4242").expect("write");

    assert_eq!(
        argv(&calls),
        vec![vec!["set", "-g", "@nerve_surface_pid", "4242"]]
    );
}
