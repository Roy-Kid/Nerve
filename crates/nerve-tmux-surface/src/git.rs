//! Git summary for the bottom panel — one `status --porcelain` instead of five
//! subprocesses (branch, ahead/behind, staged, unstaged, untracked).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitSnapshot {
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub staged: Vec<String>,
    pub unstaged: Vec<String>,
    pub untracked: Vec<String>,
    pub error: Option<String>,
}

impl GitSnapshot {
    /// Lines [`crate::ui`] renders for this snapshot — the branch row plus one
    /// row per changed file, or the error on its own.
    pub fn line_count(&self) -> usize {
        if self.error.is_some() {
            return 1;
        }
        1 + self.staged.len() + self.unstaged.len() + self.untracked.len()
    }
}

/// Rows the Git panel can show per section. `ls-files --others` on a large
/// worktree is unbounded; nothing past this is ever rendered.
pub const LIST_CAP: usize = 6;

pub fn snapshot_for_path(path: &Path) -> GitSnapshot {
    let Some(repo) = find_git_root(path) else {
        return GitSnapshot {
            error: Some("not a git repo".to_string()),
            ..GitSnapshot::default()
        };
    };
    match git_output(
        &repo,
        &["status", "--porcelain=v1", "-b", "--untracked-files=normal"],
    ) {
        Some(text) => parse_porcelain(&text),
        None => GitSnapshot {
            error: Some("git status failed".to_string()),
            ..GitSnapshot::default()
        },
    }
}

fn find_git_root(mut path: &Path) -> Option<PathBuf> {
    loop {
        if path.join(".git").exists() {
            return Some(path.to_path_buf());
        }
        path = path.parent()?;
    }
}

fn git_output(repo: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new(git_binary())
        .args(args)
        .current_dir(repo)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_binary() -> &'static str {
    static BIN: OnceLock<String> = OnceLock::new();
    BIN.get_or_init(|| {
        for candidate in [
            "/opt/homebrew/bin/git",
            "/usr/local/bin/git",
            "/usr/bin/git",
        ] {
            if Path::new(candidate).is_file() {
                return candidate.to_string();
            }
        }
        "git".to_string()
    })
    .as_str()
}

/// Porcelain v1 with `-b`: a `##` header, then `XY path` rows.
pub(crate) fn parse_porcelain(text: &str) -> GitSnapshot {
    let mut snapshot = GitSnapshot::default();
    for line in text.lines() {
        if let Some(header) = line.strip_prefix("## ") {
            parse_branch_header(header, &mut snapshot);
            continue;
        }
        if line.len() < 3 {
            continue;
        }
        let index = line.as_bytes()[0];
        let worktree = line.as_bytes()[1];
        let path = line[2..].trim();
        if path.is_empty() {
            continue;
        }
        if index == b'?' && worktree == b'?' {
            push_capped(&mut snapshot.untracked, format!("? {path}"));
            continue;
        }
        if index != b' ' && index != b'?' {
            push_capped(&mut snapshot.staged, format!("{}\t{path}", index as char));
        }
        if worktree != b' ' && worktree != b'?' {
            push_capped(
                &mut snapshot.unstaged,
                format!("{}\t{path}", worktree as char),
            );
        }
    }
    if snapshot.branch.is_empty() {
        snapshot.branch = "-".into();
    }
    snapshot
}

fn parse_branch_header(header: &str, snapshot: &mut GitSnapshot) {
    let name = header
        .split_once("...")
        .map(|(head, _)| head)
        .unwrap_or(header);
    let name = name.split_whitespace().next().unwrap_or(name).trim();
    if !name.is_empty() {
        snapshot.branch = name.to_string();
    }
    if let Some(start) = header.find('[') {
        let inside = header[start + 1..].trim_end_matches(']');
        snapshot.ahead = field_count(inside, "ahead");
        snapshot.behind = field_count(inside, "behind");
    }
}

fn field_count(inside: &str, label: &str) -> u32 {
    let mut tokens = inside.split(|c: char| c == ',' || c.is_whitespace());
    while let Some(token) = tokens.next() {
        if token == label
            && let Some(n) = tokens.next().and_then(|n| n.parse().ok())
        {
            return n;
        }
    }
    0
}

fn push_capped(rows: &mut Vec<String>, row: String) {
    if rows.len() < LIST_CAP {
        rows.push(row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_fills_branch_ahead_behind_and_three_lists() {
        let snapshot = parse_porcelain(
            "\
## main...origin/main [ahead 2, behind 1]
M  staged.rs
 M unstaged.rs
MM both.rs
?? untracked.rs
",
        );
        assert_eq!(snapshot.branch, "main");
        assert_eq!(snapshot.ahead, 2);
        assert_eq!(snapshot.behind, 1);
        assert_eq!(snapshot.staged, vec!["M\tstaged.rs", "M\tboth.rs"]);
        assert_eq!(snapshot.unstaged, vec!["M\tunstaged.rs", "M\tboth.rs"]);
        assert_eq!(snapshot.untracked, vec!["? untracked.rs"]);
        assert_eq!(snapshot.error, None);
    }
}
