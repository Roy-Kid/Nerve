//! One snapshot of the process table.
//!
//! A pane scan used to spawn `ps` once per pid (`tty_of_pid`) and once per ssh
//! pane (`destination_on_tty`). Both questions are answered by the same listing,
//! so the scan takes one process and an in-memory index.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// How long a snapshot is reused on this thread.
///
/// A sidebar tick is 200 ms; a scan that asks three questions in one tick
/// must see one table. A second later the world may have moved.
const TTL: Duration = Duration::from_millis(250);

thread_local! {
    static CACHE: RefCell<Option<(Instant, ProcTable)>> = const { RefCell::new(None) };
}

/// pid → tty (normalised) and argv, plus tty → pids.
#[derive(Clone, Debug, Default)]
pub struct ProcTable {
    by_pid: HashMap<u32, Proc>,
    by_tty: HashMap<String, Vec<u32>>,
}

#[derive(Clone, Debug)]
struct Proc {
    tty: String,
    args: String,
}

impl ProcTable {
    /// Borrow the live table. A hit does not clone every argv.
    ///
    /// `body` must not call `with_current` while a refresh is in progress on
    /// this thread: the cache is a `RefCell`, and the mutable insert is dropped
    /// before `body` runs so a nested hit can borrow again.
    pub fn with_current<R>(body: impl FnOnce(&ProcTable) -> R) -> R {
        CACHE.with(|cache| {
            let fresh = match cache.borrow().as_ref() {
                Some((at, _)) => at.elapsed() >= TTL,
                None => true,
            };
            if fresh {
                let table = snapshot();
                *cache.borrow_mut() = Some((Instant::now(), table));
            }
            let slot = cache.borrow();
            let table = &slot.as_ref().expect("table was just stored").1;
            body(table)
        })
    }

    /// Controlling terminal of `pid`, normalised (`s001`, `pts/3`).
    pub fn tty_of(&self, pid: u32) -> Option<&str> {
        let tty = self.by_pid.get(&pid).map(|proc| proc.tty.as_str())?;
        (!tty.is_empty()).then_some(tty)
    }

    /// Every pid whose controlling terminal is `tty`.
    pub fn pids_on_tty(&self, tty: &str) -> Vec<u32> {
        let key = normalize_tty(tty);
        if key.is_empty() {
            return Vec::new();
        }
        self.by_tty.get(&key).cloned().unwrap_or_default()
    }

    /// First ssh/mosh destination among processes on `tty`.
    pub fn destination_on_tty(&self, tty: &str) -> Option<String> {
        let key = normalize_tty(tty);
        let pids = self.by_tty.get(&key)?;
        pids.iter().find_map(|pid| {
            let proc = self.by_pid.get(pid)?;
            crate::ssh::parse_destination(&proc.args).map(str::to_string)
        })
    }
}

fn snapshot() -> ProcTable {
    let output = match Command::new(ps_binary())
        .args(["-ax", "-o", "pid=", "-o", "tty=", "-o", "command="])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return ProcTable::default(),
    };
    parse(&String::from_utf8_lossy(&output.stdout))
}

fn parse(text: &str) -> ProcTable {
    let mut table = ProcTable::default();
    for line in text.lines() {
        let Some((pid, tty, args)) = parse_line(line) else {
            continue;
        };
        let tty = normalize_tty(&tty);
        table.by_tty.entry(tty.clone()).or_default().push(pid);
        table.by_pid.insert(pid, Proc { tty, args });
    }
    table
}

/// ` 12345 ttys001 -zsh` → pid, raw tty, argv.
fn parse_line(line: &str) -> Option<(u32, String, String)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let pid_end = line.find(|c: char| c.is_whitespace())?;
    let pid = line[..pid_end].parse().ok()?;
    let rest = line[pid_end..].trim_start();
    let tty_end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
    let tty = rest[..tty_end].to_string();
    let args = rest[tty_end..].trim_start().to_string();
    Some((pid, tty, args))
}

/// `/dev/ttys001`, `ttys001` and `s001` are one terminal; `ps` and tmux each
/// spell it their own way.
pub fn normalize_tty(tty: &str) -> String {
    let tty = tty.trim().trim_start_matches("/dev/");
    if tty.is_empty() || tty.starts_with('?') || tty == "-" {
        return String::new();
    }
    match tty.strip_prefix("tty") {
        Some(rest) if !rest.is_empty() => rest.to_string(),
        _ => tty.to_string(),
    }
}

fn ps_binary() -> &'static str {
    static BIN: OnceLock<String> = OnceLock::new();
    BIN.get_or_init(|| {
        for candidate in ["/bin/ps", "/usr/bin/ps"] {
            if Path::new(candidate).is_file() {
                return candidate.to_string();
            }
        }
        "ps".to_string()
    })
    .as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_listing_indexes_pid_and_tty() {
        let table = parse(
            "\
  1 ??         /sbin/launchd
  449 ttys000    -zsh
  512 ttys000    ssh Arrhenius
  880 pts/3      claude
",
        );
        assert_eq!(table.tty_of(449), Some("s000"));
        assert_eq!(table.tty_of(1), None);
        assert_eq!(table.pids_on_tty("ttys000"), vec![449, 512]);
        assert_eq!(table.pids_on_tty("/dev/pts/3"), vec![880]);
        assert_eq!(
            table.destination_on_tty("/dev/ttys000").as_deref(),
            Some("Arrhenius")
        );
        assert_eq!(table.destination_on_tty("pts/3"), None);
    }

    #[test]
    fn tty_spellings_normalise_to_one() {
        assert_eq!(normalize_tty("/dev/ttys001"), "s001");
        assert_eq!(normalize_tty("ttys001"), "s001");
        assert_eq!(normalize_tty("s001"), "s001");
        assert_eq!(normalize_tty("/dev/pts/3"), "pts/3");
        assert_eq!(normalize_tty("pts/3"), "pts/3");
        assert_eq!(normalize_tty("??"), "");
        assert_eq!(normalize_tty("-"), "");
    }
}
