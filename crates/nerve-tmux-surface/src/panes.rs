//! Which pane belongs to a job.
//!
//! Identity first, heuristics after: the hook reports the agent process id, and
//! that process sits on exactly one pane's tty. Paths, session names and
//! locality only break ties between panes that could not be identified that
//! way.
//!
//! That whole scan assumes the job runs on this machine. A job reported by a
//! producer on another host — its jobs arrive through the RemoteForward tunnel
//! like any other (CLAUDE.md invariant 5) — has no pane here at all, and its
//! pid and workspace path describe another machine's process and another
//! machine's filesystem. Those are matched instead by the session the human is
//! watching them through: the pane running `ssh` to that host (`crate::ssh`),
//! which is a route to the *machine* — see [`SshRoute`].
//!
//! Deciding only. Nothing here moves a pane or paints anything; what the
//! sidebar does with the answer is [`crate::preview`].

use std::env;
use std::path::{Path, PathBuf};

use crate::procs::{self, normalize_tty};
use crate::ssh::{self, SshHosts};
use crate::tmux::{self, SIDEBAR_ROLE};
use nerve_surface_core::frame::JobView;
use nerve_surface_core::jobpath::job_path;
use nerve_surface_core::machine;

const LIST_PANES_FORMAT: &str = concat!(
    "#{pane_id}\t#{pane_current_path}\t#{@nerve_pane_role}\t",
    "#{pane_tty}\t#{session_name}\t#{window_id}\t#{pane_current_command}\t",
    "#{@nerve_preview_owner}"
);

/// Where the sidebar itself lives. Fixed for the process's lifetime, so it is
/// read once instead of on every pane scan.
pub(crate) struct SidebarPane {
    pub(crate) id: String,
    pub(crate) session: String,
    pub(crate) window: String,
}

impl SidebarPane {
    /// No pane at all — the shape [`Self::detect`] answers outside tmux.
    pub(crate) fn none() -> Self {
        Self {
            id: String::new(),
            session: String::new(),
            window: String::new(),
        }
    }

    pub(crate) fn detect() -> Self {
        let id = env::var("TMUX_PANE").unwrap_or_default();
        if id.is_empty() {
            return Self::none();
        }
        let info = tmux::display_message(&id, "#{session_name}\t#{window_id}");
        let (session, window) = info.split_once('\t').unwrap_or(("", ""));
        Self {
            session: session.to_string(),
            window: window.to_string(),
            id,
        }
    }
}

/// One row of [`LIST_PANES_FORMAT`].
struct PaneRow<'a> {
    id: &'a str,
    path: &'a str,
    role: &'a str,
    tty: &'a str,
    session: &'a str,
    window: &'a str,
    /// Foreground process, which is how an ssh session announces itself.
    command: &'a str,
    /// Sidebar currently showing this pane, if any ([`tmux::PREVIEW_OWNER`]).
    owner: &'a str,
}

impl<'a> PaneRow<'a> {
    /// Whether another sidebar is already showing this pane.
    ///
    /// Two sidebars taking turns on one agent pane is not a race anyone can
    /// win: each swap moves it out of the other's slot, and both then hold a
    /// record that puts the wrong pane back.
    fn taken_by_another(&self, sidebar: &SidebarPane) -> bool {
        !self.owner.is_empty() && self.owner != sidebar.id
    }

    fn parse(line: &'a str) -> Option<Self> {
        let mut parts = line.split('\t');
        Some(Self {
            id: parts.next()?,
            path: parts.next().unwrap_or(""),
            role: parts.next().unwrap_or(""),
            tty: parts.next().unwrap_or(""),
            session: parts.next().unwrap_or(""),
            window: parts.next().unwrap_or(""),
            command: parts.next().unwrap_or(""),
            owner: parts.next().unwrap_or(""),
        })
    }
}

/// How well one pane answers "is this the job's terminal", best-first.
///
/// Fields are compared in declaration order, so `tty` — the pane whose tty
/// carries the agent process — beats every heuristic under it. Without it a
/// content pane sitting next to the sidebar in the same project directory
/// would outrank the agent's real pane in another window.
///
/// `tty`/`path` are what a local job is matched on and `host` is what a remote
/// one is matched on; a scan is one or the other, never both, so the two never
/// compete inside a single comparison.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct PaneRank {
    /// The agent process runs on this pane's tty.
    tty: bool,
    /// How well this pane's ssh session names the job's machine
    /// ([`ssh::host_score`]).
    host: u8,
    /// Filesystem overlap between the pane's cwd and the job's workspace.
    path: usize,
    /// tmux session named after the job.
    named: bool,
    /// Same tmux session as the sidebar.
    session: bool,
    /// Same tmux window as the sidebar.
    window: bool,
}

/// What one job looks like to the pane scan.
///
/// Which arm applies is decided by machine identity alone, never by what
/// happened to match: a remote job's pid names a process on another machine and
/// its workspace names another machine's directory, and either can collide with
/// a local pane by accident. `crates/nerve-hub/src/state/reaper.rs` refuses
/// remote pids for exactly the same reason.
pub(crate) enum JobTarget {
    /// The job runs here: match the pane the agent itself sits on.
    Local {
        workspace: Option<PathBuf>,
        /// Normalised tty of the job's agent process (`s001`, `pts/3`).
        tty: Option<String>,
    },
    /// The job runs on another machine: the closest thing here is the pane
    /// holding an ssh session to it.
    Remote { alias: String },
}

impl JobTarget {
    /// What to scan for, or `None` when the job says nothing this machine can
    /// act on. `local` is this machine's alias ([`machine::local_alias`]).
    pub(crate) fn of(job: &JobView, local: Option<&str>) -> Option<Self> {
        if let Some(alias) = machine::foreign_alias(&job.alias, local) {
            return Some(Self::Remote {
                alias: alias.to_string(),
            });
        }
        let workspace = job_path(job).map(|path| normalize_path(&path));
        let tty = job.extensions.pid.and_then(|pid| {
            procs::ProcTable::with_current(|table| table.tty_of(pid).map(str::to_string))
        });
        // Nothing to match on — the job never said where it runs.
        (workspace.is_some() || tty.is_some()).then_some(Self::Local { workspace, tty })
    }
}

pub(crate) fn find_pane_for_job(job: &JobView, sidebar: &SidebarPane) -> Option<String> {
    let target = JobTarget::of(job, machine::local_alias())?;
    find_pane_for_target(&target, job, sidebar)
}

/// The pane a target resolves to, scanning every pane on the server.
pub(crate) fn find_pane_for_target(
    target: &JobTarget,
    job: &JobView,
    sidebar: &SidebarPane,
) -> Option<String> {
    let panes = tmux::run_tmux(&["list-panes", "-a", "-F", LIST_PANES_FORMAT])?;
    match target {
        JobTarget::Local { workspace, tty } => select_local_pane(
            panes.as_str(),
            job,
            sidebar,
            workspace.as_deref(),
            tty.as_deref(),
        ),
        JobTarget::Remote { alias } => select_ssh_route(
            panes.as_str(),
            sidebar,
            alias,
            SshHosts::load(),
            &ssh::destination_on_tty,
        )
        .map(|route| route.pane),
    }
}

/// The ssh route for a job on another machine, pane and host together.
pub(crate) fn find_ssh_route(alias: &str, sidebar: &SidebarPane) -> Option<SshRoute> {
    let panes = tmux::run_tmux(&["list-panes", "-a", "-F", LIST_PANES_FORMAT])?;
    select_ssh_route(
        &panes,
        sidebar,
        alias,
        SshHosts::load(),
        &ssh::destination_on_tty,
    )
}

/// Best pane among `panes` ([`LIST_PANES_FORMAT`] rows, one per line) for a job
/// running on this machine.
///
/// A row that matches nothing is skipped, never fatal: `list-panes -a` reports
/// every pane on the server and all but one of them are unrelated by design.
fn select_local_pane(
    panes: &str,
    job: &JobView,
    sidebar: &SidebarPane,
    workspace: Option<&Path>,
    agent_tty: Option<&str>,
) -> Option<String> {
    best_pane(panes, sidebar, |pane| {
        let path = workspace.and_then(|workspace| path_match_score(pane.path, workspace));
        let tty = agent_tty.is_some_and(|tty| normalize_tty(pane.tty) == tty);
        if path.is_none() && !tty {
            return None;
        }
        Some(PaneRank {
            tty,
            path: path.unwrap_or(0),
            named: !job.name.is_empty() && pane.session == job.name,
            ..locality(pane, sidebar)
        })
    })
}

/// The pane holding an ssh session to `alias`, and the host it reaches.
///
/// Both halves are needed: the pane is where `Enter` lands the human, and the
/// destination is what [`crate::remote`] then talks to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshRoute {
    pub pane: String,
    pub destination: String,
    /// Window the pane sits in — the sidebar only steers a far side it can
    /// actually see (`crate::preview`).
    pub window: String,
}

/// Best pane holding an ssh session to `alias`.
///
/// `destination` is injected so the scan is testable without a process table:
/// in the binary it is [`ssh::destination_on_tty`], one `ps` per pane that is
/// already known to be running a client.
fn select_ssh_route(
    panes: &str,
    sidebar: &SidebarPane,
    alias: &str,
    hosts: &SshHosts,
    destination: &dyn Fn(&str) -> Option<String>,
) -> Option<SshRoute> {
    let mut best: Option<(PaneRank, SshRoute)> = None;
    for line in panes.lines() {
        let Some(pane) = PaneRow::parse(line) else {
            continue;
        };
        if pane.id == sidebar.id || pane.role == SIDEBAR_ROLE || pane.taken_by_another(sidebar) {
            continue;
        }
        if !ssh::is_remote_client(pane.command) {
            continue;
        }
        let Some(reaches) = destination(pane.tty) else {
            continue;
        };
        let Some(host) = ssh::host_score(&reaches, alias, hosts) else {
            continue;
        };
        let rank = PaneRank {
            host,
            ..locality(&pane, sidebar)
        };
        if best.as_ref().is_none_or(|(prev, _)| rank > *prev) {
            best = Some((
                rank,
                SshRoute {
                    pane: pane.id.to_string(),
                    destination: reaches,
                    window: pane.window.to_string(),
                },
            ));
        }
    }
    best.map(|(_, route)| route)
}

/// Walk the scan once, keeping the best-ranked pane `rank` accepts.
fn best_pane(
    panes: &str,
    sidebar: &SidebarPane,
    rank: impl Fn(&PaneRow<'_>) -> Option<PaneRank>,
) -> Option<String> {
    let mut best: Option<(PaneRank, String)> = None;
    for line in panes.lines() {
        let Some(pane) = PaneRow::parse(line) else {
            continue;
        };
        if pane.id == sidebar.id || pane.role == SIDEBAR_ROLE || pane.taken_by_another(sidebar) {
            continue;
        }
        let Some(rank) = rank(&pane) else {
            continue;
        };
        if best.as_ref().is_none_or(|(prev, _)| rank > *prev) {
            best = Some((rank, pane.id.to_string()));
        }
    }
    best.map(|(_, pane)| pane)
}

/// How close a pane sits to the sidebar — the tie-break under every real match.
fn locality(pane: &PaneRow<'_>, sidebar: &SidebarPane) -> PaneRank {
    PaneRank {
        session: !sidebar.session.is_empty() && pane.session == sidebar.session,
        window: !sidebar.window.is_empty() && pane.window == sidebar.window,
        ..PaneRank::default()
    }
}

/// Every pid on a pane's terminal — the reverse of the pid → tty lookup.
///
/// One process-table snapshot for the whole pane rather than one `ps` per job:
/// the question is "who is running here", and the answer is a handful of pids
/// the caller matches against what the hub reported.
pub(crate) fn pids_on_tty(tty: &str) -> Vec<u32> {
    procs::ProcTable::with_current(|table| table.pids_on_tty(tty))
}

fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Higher is better. `None` means no relationship.
pub fn path_match_score(pane_path: &str, target: &Path) -> Option<usize> {
    if pane_path.is_empty() {
        return None;
    }
    let pane_s = Path::new(pane_path).to_string_lossy();
    let target_s = target.to_string_lossy();
    if pane_s == target_s {
        return Some(usize::MAX);
    }
    if target_s.starts_with(pane_s.as_ref()) || pane_s.starts_with(target_s.as_ref()) {
        return Some(pane_s.len().min(target_s.len()));
    }
    let pane = normalize_path(Path::new(pane_path));
    if pane == target {
        return Some(usize::MAX);
    }
    let pane_canon = pane.to_string_lossy();
    let target_canon = target.to_string_lossy();
    if target_canon.starts_with(pane_canon.as_ref())
        || pane_canon.starts_with(target_canon.as_ref())
    {
        return Some(pane_canon.len().min(target_canon.len()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job_named(name: &str) -> JobView {
        serde_json::from_value(serde_json::json!({ "id": "codex:s1", "name": name }))
            .expect("job fixture")
    }
    fn sidebar() -> SidebarPane {
        SidebarPane {
            id: "%9".to_string(),
            session: "nerve".to_string(),
            window: "@1".to_string(),
        }
    }
    /// `#{pane_id}\t#{path}\t#{role}\t#{tty}\t#{session}\t#{window}\t#{command}`
    fn row(id: &str, path: &str, tty: &str, session: &str, window: &str) -> String {
        format!("{id}\t{path}\t\t/dev/{tty}\t{session}\t{window}\tzsh\t")
    }

    /// The same row, already claimed by the sidebar `owner`.
    fn owned_row(id: &str, path: &str, tty: &str, owner: &str) -> String {
        format!("{id}\t{path}\t\t/dev/{tty}\tagents\t@8\tzsh\t{owner}")
    }
    /// The same row, for a pane whose foreground process is an ssh client.
    fn ssh_row(id: &str, tty: &str, session: &str, window: &str) -> String {
        format!("{id}\t/Users/me\t\t/dev/{tty}\t{session}\t{window}\tssh\t")
    }
    fn local(
        panes: &str,
        job: &JobView,
        workspace: Option<&str>,
        tty: Option<&str>,
    ) -> Option<String> {
        select_local_pane(
            panes,
            job,
            &sidebar(),
            workspace.map(Path::new),
            tty.map(normalize_tty).as_deref(),
        )
    }
    /// The pane `arrhenius1` routes to in this scan, if any.
    fn route_pane(panes: &str, seen: &dyn Fn(&str) -> Option<String>) -> Option<String> {
        select_ssh_route(panes, &sidebar(), "arrhenius1", &SshHosts::default(), seen)
            .map(|route| route.pane)
    }
    fn destinations(
        pairs: &'static [(&'static str, &'static str)],
    ) -> impl Fn(&str) -> Option<String> {
        move |tty: &str| {
            pairs
                .iter()
                .find(|(pane_tty, _)| *pane_tty == tty)
                .map(|(_, destination)| destination.to_string())
        }
    }
    #[test]
    fn exact_path_scores_highest() {
        let target = PathBuf::from("/tmp/work");
        assert_eq!(path_match_score("/tmp/work", &target), Some(usize::MAX));
    }
    #[test]
    fn prefix_paths_score_by_overlap() {
        let target = PathBuf::from("/tmp/work/nerve");
        assert_eq!(
            path_match_score("/tmp/work", &target),
            Some("/tmp/work".len())
        );
    }
    #[test]
    fn unrelated_paths_do_not_match() {
        let target = PathBuf::from("/tmp/work");
        assert_eq!(path_match_score("/elsewhere", &target), None);
    }
    #[test]
    fn unrelated_panes_do_not_end_the_scan() {
        let panes = [
            row("%1", "/elsewhere", "ttys001", "other", "@7"),
            row("%2", "", "ttys002", "other", "@7"),
            row("%3", "/tmp/work", "ttys003", "agents", "@8"),
        ]
        .join("\n");
        assert_eq!(
            local(&panes, &job_named("work"), Some("/tmp/work"), None),
            Some("%3".to_string())
        );
    }
    #[test]
    fn agent_tty_beats_a_same_window_path_match() {
        // The sidebar's own content pane sits in the job's directory; the real
        // agent runs in another window. Identity has to win.
        let panes = [
            row("%2", "/tmp/work", "ttys002", "nerve", "@1"),
            row("%5", "/tmp/work", "ttys009", "agents", "@8"),
        ]
        .join("\n");
        assert_eq!(
            local(&panes, &job_named("work"), Some("/tmp/work"), Some("s009")),
            Some("%5".to_string())
        );
    }
    #[test]
    fn agent_tty_matches_without_any_path() {
        let panes = ssh_row("%5", "ttys009", "agents", "@8").replace("ssh", "zsh");
        assert_eq!(
            local(&panes, &job_named("work"), None, Some("s009")),
            Some("%5".to_string())
        );
    }
    #[test]
    fn sidebar_and_sidebar_roled_panes_are_never_targets() {
        let panes = [
            row("%9", "/tmp/work", "ttys009", "nerve", "@1"),
            format!("%8\t/tmp/work\t{SIDEBAR_ROLE}\t/dev/ttys008\tnerve\t@1\tzsh\t"),
        ]
        .join("\n");
        assert_eq!(
            local(&panes, &job_named("work"), Some("/tmp/work"), None),
            None
        );
    }
    #[test]
    fn a_pane_another_sidebar_is_showing_is_left_alone() {
        // The same agent pane, already pulled into the other window's slot.
        let panes = [
            owned_row("%5", "/tmp/work", "ttys005", "%77"),
            row("%6", "/tmp/work", "ttys006", "agents", "@8"),
        ]
        .join("\n");
        assert_eq!(
            local(&panes, &job_named("work"), Some("/tmp/work"), None),
            Some("%6".to_string()),
            "two sidebars must not take turns yanking one pane"
        );

        // Our own claim is not someone else's: re-showing what we already show
        // has to keep finding it.
        let mine = owned_row("%5", "/tmp/work", "ttys005", "%9");
        assert_eq!(
            local(&mine, &job_named("work"), Some("/tmp/work"), None),
            Some("%5".to_string())
        );
    }

    #[test]
    fn same_window_breaks_ties_between_equal_paths() {
        let panes = [
            row("%1", "/tmp/work", "ttys001", "other", "@7"),
            row("%2", "/tmp/work", "ttys002", "nerve", "@1"),
        ]
        .join("\n");
        assert_eq!(
            local(&panes, &job_named("work"), Some("/tmp/work"), None),
            Some("%2".to_string())
        );
    }
    #[test]
    fn a_remote_job_lands_on_the_pane_ssh_d_into_its_machine() {
        let panes = [
            row("%1", "/Users/me/work", "ttys001", "nerve", "@1"),
            ssh_row("%4", "ttys006", "other", "@4"),
            ssh_row("%7", "ttys010", "other", "@5"),
        ]
        .join("\n");
        let seen = destinations(&[("/dev/ttys006", "Arrhenius"), ("/dev/ttys010", "dardel")]);
        assert_eq!(route_pane(&panes, &seen), Some("%4".to_string()));
    }
    #[test]
    fn a_machine_nobody_is_ssh_d_into_matches_nothing() {
        let panes = ssh_row("%4", "ttys006", "other", "@4");
        let seen = destinations(&[("/dev/ttys006", "dardel")]);
        assert_eq!(route_pane(&panes, &seen), None);
    }
    #[test]
    fn an_exact_host_outranks_a_prefix_of_it() {
        let panes = [
            ssh_row("%4", "ttys006", "other", "@4"),
            ssh_row("%7", "ttys010", "other", "@5"),
        ]
        .join("\n");
        let seen = destinations(&[
            ("/dev/ttys006", "Arrhenius"),
            ("/dev/ttys010", "arrhenius1"),
        ]);
        assert_eq!(route_pane(&panes, &seen), Some("%7".to_string()));
    }
    #[test]
    fn the_route_carries_the_host_enter_has_to_talk_to() {
        let panes = ssh_row("%4", "ttys006", "other", "@4");
        let seen = destinations(&[("/dev/ttys006", "Arrhenius")]);
        let route = select_ssh_route(
            &panes,
            &sidebar(),
            "arrhenius1",
            &SshHosts::default(),
            &seen,
        )
        .expect("route");
        assert_eq!(route.pane, "%4");
        // The pane alone is not enough: `Enter` has to address the far side
        // exactly as the human's config spells it (`ssh::parse_destination`
        // already strips any `user@` before it gets here).
        assert_eq!(route.destination, "Arrhenius");
    }
    /// Why the preview refuses to mirror an ssh pane: the route is the
    /// machine's, so every job on that machine resolves to the same pane and a
    /// mirror would show three jobs one terminal and call it each of them.
    #[test]
    fn every_job_on_one_machine_resolves_to_the_same_ssh_pane() {
        let panes = [
            ssh_row("%4", "ttys006", "other", "@4"),
            row("%1", "/Users/me/work", "ttys001", "nerve", "@1"),
        ]
        .join("\n");
        let seen = destinations(&[("/dev/ttys006", "Arrhenius")]);
        let pane_for = |_job: &str| route_pane(&panes, &seen);
        assert_eq!(pane_for("molcrafts"), Some("%4".to_string()));
        assert_eq!(pane_for("salamander"), pane_for("molcrafts"));
    }
    #[test]
    fn a_remote_job_never_scans_for_a_local_pane() {
        // Same pid as a live local process, same-looking workspace: neither may
        // be consulted, because neither describes this machine.
        let job: JobView = serde_json::from_value(serde_json::json!({
            "id": "claude-code:s1",
            "alias": "arrhenius1",
            "name": "molcrafts",
            "extensions": { "pid": std::process::id() },
            "context": { "workspace": "/tmp" }
        }))
        .expect("job fixture");
        let JobTarget::Remote { alias } =
            JobTarget::of(&job, Some("RoydeMacBook-Air")).expect("target")
        else {
            panic!("a job on another machine must never be matched locally");
        };
        assert_eq!(alias, "arrhenius1");
    }
    #[test]
    fn tty_spellings_normalise_to_one() {
        assert_eq!(normalize_tty("/dev/ttys001"), "s001");
        assert_eq!(normalize_tty("ttys001"), "s001");
        assert_eq!(normalize_tty("s001"), "s001");
        assert_eq!(normalize_tty("/dev/pts/3"), "pts/3");
        assert_eq!(normalize_tty("pts/3"), "pts/3");
    }
}
