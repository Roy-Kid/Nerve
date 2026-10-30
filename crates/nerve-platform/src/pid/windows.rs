//! `OpenProcess` + `GetExitCodeProcess` — the Windows form of the same question.
//!
//! The one place in the repo that calls Win32 directly, which is why this crate
//! opts out of the workspace's `unsafe_code = "forbid"` (see its manifest).
//! `sysinfo` would answer the same question safely, but it pulls the whole
//! `windows` crate on Windows and `objc2-core-foundation` on macOS, into a
//! release profile whose whole point is size.
//!
//! Two documented differences from the POSIX probe, both inside the existing
//! fail-open bias:
//!
//! - A process that genuinely exits with code `259` is indistinguishable from
//!   a running one, because `259` *is* `STILL_ACTIVE`. It reads as alive until
//!   something else closes the row.
//! - Windows recycles pids far faster than POSIX, so [`state`] can answer
//!   about a different process with the same number. [`start_token`] is that
//!   process's creation time; a caller reaps when it no longer matches the
//!   one it recorded. A token we cannot read is not evidence of death.

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, FILETIME, GetLastError, HANDLE,
    STILL_ACTIVE,
};
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

use super::PidState;

/// Closes the handle on every path out, so the `unsafe` block below has one
/// exit and cannot leak on an early return.
struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: `self.0` came from a successful `OpenProcess` and is closed
        // exactly once, here, because `OwnedHandle` is neither `Copy` nor
        // `Clone` and is never constructed from a null handle.
        #[allow(unsafe_code)]
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub fn state(pid: i32) -> PidState {
    let Ok(pid) = u32::try_from(pid) else {
        // Negative pids name a process group on POSIX and nothing at all here.
        return PidState::Denied;
    };

    // SAFETY: `OpenProcess` takes plain integers and returns either a handle we
    // own or null; nothing is dereferenced. `GetExitCodeProcess` writes one
    // `u32` through a pointer to a live local. The handle is released by
    // `OwnedHandle`'s `Drop`.
    #[allow(unsafe_code)]
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return match GetLastError() {
                ERROR_INVALID_PARAMETER => PidState::Dead,
                // It exists; we simply may not look. Same meaning as `EPERM`.
                ERROR_ACCESS_DENIED => PidState::Denied,
                // An answer we do not understand is not evidence of death.
                _ => PidState::Denied,
            };
        }
        let handle = OwnedHandle(handle);

        let mut code: u32 = 0;
        if GetExitCodeProcess(handle.0, &mut code) == 0 {
            return PidState::Denied;
        }
        if code == STILL_ACTIVE as u32 {
            PidState::Alive
        } else {
            PidState::Dead
        }
    }
}

/// Creation time of `pid` as an opaque decimal token.
///
/// `None` when the process cannot be queried. That is not proof it died;
/// [`state`] is the liveness answer. Does not signal the process.
pub fn start_token(pid: i32) -> Option<String> {
    let Ok(pid) = u32::try_from(pid) else {
        return None;
    };

    // SAFETY: same handle contract as `state`. `GetProcessTimes` writes four
    // `FILETIME`s through pointers to live locals. The handle is released by
    // `OwnedHandle`'s `Drop`. Nothing here signals the process.
    #[allow(unsafe_code)]
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let handle = OwnedHandle(handle);
        let mut created = FILETIME::default();
        let mut exited = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        if GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) == 0 {
            return None;
        }
        let ticks = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
        if ticks == 0 {
            None
        } else {
            Some(ticks.to_string())
        }
    }
}
