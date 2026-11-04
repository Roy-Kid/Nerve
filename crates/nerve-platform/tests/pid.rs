//! Liveness, asked on whichever OS is running the suite.
//!
//! Every case here is written to mean the same thing on unix and on Windows, so
//! the Windows CI job exercises the Win32 probe with no separate suite.

use std::process::Command;

use nerve_platform::pid::{PidState, is_alive, start_token, state};

/// A child that has been waited on. On Windows the handle is only released by
/// the `wait`, which is the one place the two OSes differ in observable timing
/// — so the wait is the assertion, not an implementation detail.
fn reaped_child_pid() -> i32 {
    let mut command = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", "exit"]);
        c
    } else {
        Command::new("true")
    };
    let mut child = command.spawn().expect("spawn a process that exits at once");
    let pid = child.id() as i32;
    child.wait().expect("wait for it");
    pid
}

#[test]
fn our_own_process_is_alive() {
    let me = std::process::id() as i32;
    assert_eq!(state(me), PidState::Alive);
    assert!(is_alive(me));
}

#[test]
fn a_reaped_child_is_dead() {
    let pid = reaped_child_pid();
    assert_eq!(state(pid), PidState::Dead);
    assert!(!is_alive(pid));
}

#[cfg(unix)]
#[test]
fn init_exists_even_when_we_may_not_signal_it() {
    // pid 1 is always running and usually not ours to signal — exactly the
    // case the `Denied` state exists for.
    assert_ne!(state(1), PidState::Dead);
}

/// Kills the child on every path out, including a failed assertion.
struct ReapChild(Option<std::process::Child>);

impl Drop for ReapChild {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[test]
fn a_child_start_token_differs_from_ours_while_it_lives() {
    let mut command = if cfg!(windows) {
        let mut c = Command::new("ping");
        c.args(["-n", "31", "127.0.0.1"]);
        c
    } else {
        let mut c = Command::new("sleep");
        c.arg("30");
        c
    };
    let child = command.spawn().expect("spawn a process that stays up");
    let child = ReapChild(Some(child));
    let pid = child.0.as_ref().expect("child").id() as i32;
    let token = start_token(pid);
    assert!(token.is_some(), "pid {pid}");
    assert_eq!(start_token(pid), token);
    assert_ne!(token, start_token(std::process::id() as i32));
}
