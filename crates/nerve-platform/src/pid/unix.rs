//! `kill(pid, 0)` — the POSIX "does this process exist" question.
//!
//! Sends no signal. It only asks the kernel to validate the pid and our right
//! to signal it. The start token is a separate read (`proc_pidinfo` on macOS,
//! `/proc/<pid>/stat` on Linux) so a recycled pid is not mistaken for the
//! process that reported.

use super::PidState;

pub fn state(pid: i32) -> PidState {
    // Not a process id at all: 0 means "our own group" and negatives mean a
    // group. Callers already refuse those, but the guard cannot be left to
    // `Pid::from_raw` — it debug-asserts `raw >= 0` rather than answering
    // `None`, so a negative pid panics instead of degrading.
    if pid <= 0 {
        return PidState::Denied;
    }
    let Some(pid) = rustix::process::Pid::from_raw(pid) else {
        return PidState::Denied;
    };
    match rustix::process::test_kill_process(pid) {
        Ok(()) => PidState::Alive,
        Err(rustix::io::Errno::SRCH) => PidState::Dead,
        Err(_) => PidState::Denied,
    }
}

/// Opaque start token. `None` is not death — the caller treats it that way.
pub fn start_token(pid: i32) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        macos::start_token(pid)
    }
    #[cfg(target_os = "linux")]
    {
        linux::start_token(pid)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = pid;
        None
    }
}

/// `starttime` (field 22) from a `/proc/<pid>/stat` line.
///
/// The comm field is wrapped in parentheses and may itself contain spaces
/// and `)`, so the fields after the last `)` are what counts.
#[cfg(any(test, target_os = "linux"))]
fn start_token_from_stat(text: &str) -> Option<String> {
    let rest = text.rsplit_once(')')?.1.trim_start();
    let start = rest.split_whitespace().nth(19)?;
    if start.is_empty() || !start.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(start.to_string())
}

#[cfg(target_os = "linux")]
mod linux {
    pub fn start_token(pid: i32) -> Option<String> {
        let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        super::start_token_from_stat(&text)
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;
    use std::mem::{MaybeUninit, offset_of, size_of};

    const PROC_PIDTBSDINFO: i32 = 3;
    const MAXCOMLEN: usize = 16;

    /// `struct proc_bsdinfo` from `<sys/proc_info.h>`. Only the pid and the
    /// start timeval are read; the rest exists so the offsets stay the kernel's.
    #[repr(C)]
    #[allow(dead_code)]
    struct ProcBsdInfo {
        pbi_flags: u32,
        pbi_status: u32,
        pbi_xstatus: u32,
        pbi_pid: u32,
        pbi_ppid: u32,
        pbi_uid: u32,
        pbi_gid: u32,
        pbi_ruid: u32,
        pbi_rgid: u32,
        pbi_svuid: u32,
        pbi_svgid: u32,
        rfu_1: u32,
        pbi_comm: [u8; MAXCOMLEN],
        pbi_name: [u8; 2 * MAXCOMLEN],
        pbi_nfiles: u32,
        pbi_pgid: u32,
        pbi_pjobc: u32,
        e_tdev: u32,
        e_tpgid: u32,
        pbi_nice: i32,
        pbi_start_tvsec: u64,
        pbi_start_tvusec: u64,
    }

    const _: () = assert!(size_of::<ProcBsdInfo>() == 136);
    const _: () = assert!(offset_of!(ProcBsdInfo, pbi_pid) == 12);
    const _: () = assert!(offset_of!(ProcBsdInfo, pbi_comm) == 48);
    const _: () = assert!(offset_of!(ProcBsdInfo, pbi_start_tvsec) == 120);
    const _: () = assert!(offset_of!(ProcBsdInfo, pbi_start_tvusec) == 128);

    #[allow(unsafe_code)]
    #[link(name = "proc")]
    unsafe extern "C" {
        fn proc_pidinfo(
            pid: i32,
            flavor: i32,
            arg: u64,
            buffer: *mut c_void,
            buffersize: i32,
        ) -> i32;
    }

    #[allow(unsafe_code)]
    pub fn start_token(pid: i32) -> Option<String> {
        // SAFETY: `proc_pidinfo` writes at most `buffersize` bytes into `info`
        // and returns how many it wrote. The buffer is exactly one
        // `proc_bsdinfo`. It is read only when the kernel reports a full
        // struct, and the call does not signal the process.
        let info = unsafe {
            let mut info = MaybeUninit::<ProcBsdInfo>::uninit();
            let wrote = proc_pidinfo(
                pid,
                PROC_PIDTBSDINFO,
                0,
                info.as_mut_ptr().cast(),
                size_of::<ProcBsdInfo>() as i32,
            );
            if wrote as usize != size_of::<ProcBsdInfo>() {
                return None;
            }
            info.assume_init()
        };
        let Ok(expected) = u32::try_from(pid) else {
            return None;
        };
        if info.pbi_pid != expected {
            return None;
        }
        Some(format!(
            "{}:{}",
            info.pbi_start_tvsec, info.pbi_start_tvusec
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn stat_starttime_survives_a_comm_that_contains_parentheses() {
        let zeros = ["0"; 18].join(" ");
        let text = format!("7 (a) b) S {zeros} 424242\n");
        assert_eq!(
            super::start_token_from_stat(&text).as_deref(),
            Some("424242")
        );
    }
}
