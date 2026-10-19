//! Panes that hold an ssh session, and which machine each one reaches.
//!
//! A job on another machine has no pane here — its agent's terminal is on that
//! machine. What *is* here is the session the human is watching it through: the
//! pane running `ssh`. That pane is where "return to the agent UI" leads, so it
//! is what `Enter` jumps to (`crate::preview::select_ssh_pane`).
//!
//! It is a route to a *machine*, not to a job: every job on that machine
//! resolves to the same pane. The preview never mirrors it for that reason —
//! three jobs sharing one terminal would each be shown as if it were theirs.
//!
//! Matching is honest about what it is. A local job is matched by identity —
//! the agent's pid sits on exactly one pane's tty. A remote job has no identity
//! to compare, only two names: the alias its producer reported (`arrhenius1`,
//! a hostname) and the destination the human typed (`ssh Arrhenius`, an
//! `~/.ssh/config` Host). [`host_score`] ranks how well those two agree and
//! never invents a match out of a shared first letter.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Commands that mean "this pane is somewhere else".
///
/// `pane_current_command` is the pane's foreground process, which stays `ssh`
/// no matter what runs on the far side — a remote tmux included.
const CLIENTS: [&str; 4] = ["ssh", "autossh", "mosh", "mosh-client"];

/// Short ssh options that consume the next argument, so the token after them is
/// never the destination.
const OPTIONS_WITH_VALUE: &str = "bcDeEFIiJLlmOopQRSWw";

/// Shortest name that may be prefix-matched, so `a` cannot stand for `arrhenius1`.
const MIN_PREFIX: usize = 3;

/// Whether a pane's foreground command is a remote-shell client.
pub fn is_remote_client(command: &str) -> bool {
    let command = command.trim().rsplit('/').next().unwrap_or_default();
    CLIENTS.contains(&command)
}

/// Where the ssh session on `tty` is connected, as the human spelled it.
///
/// Reads the same process-table snapshot the pane scan uses, so an ssh pane
/// does not cost a second `ps`.
pub fn destination_on_tty(tty: &str) -> Option<String> {
    crate::procs::ProcTable::with_current(|table| table.destination_on_tty(tty))
}

/// The destination in one `ps` line, or `None` if that line is not a session
/// this surface can send anyone back to.
///
/// Tunnels are refused on purpose: `ssh -N` has no remote terminal, and the
/// Nerve RemoteForward itself is exactly that shape (`notes.md` 2026-07-22).
pub fn parse_destination(args: &str) -> Option<&str> {
    let mut tokens = args.split_whitespace();
    if !is_remote_client(tokens.next()?) {
        return None;
    }
    let mut skip_value = false;
    for token in tokens {
        if skip_value {
            skip_value = false;
            continue;
        }
        let Some(flags) = short_flags(token) else {
            return destination_host(token);
        };
        if is_tunnel(flags) {
            return None;
        }
        skip_value = flags_take_next_token(flags);
    }
    None
}

/// The flag cluster in `-4vN`, or `None` for a token that is not a short flag.
fn short_flags(token: &str) -> Option<&str> {
    if token.starts_with("--") || token == "-" {
        return None;
    }
    token.strip_prefix('-')
}

/// Whether a cluster spends the token after it.
///
/// Read left to right, as ssh reads it: the first flag that takes a value takes
/// the rest of the token (`-p2222`), or the next token when the cluster ends
/// there (`-vp 2222`).
fn flags_take_next_token(flags: &str) -> bool {
    let mut rest = flags.chars();
    while let Some(flag) = rest.next() {
        if OPTIONS_WITH_VALUE.contains(flag) {
            return rest.as_str().is_empty();
        }
    }
    false
}

/// Whether the cluster asks for a connection with no remote terminal (`-N`).
fn is_tunnel(flags: &str) -> bool {
    for flag in flags.chars() {
        // Past a value-taking flag the characters are that value, not flags.
        if OPTIONS_WITH_VALUE.contains(flag) {
            return false;
        }
        if flag == 'N' {
            return true;
        }
    }
    false
}

/// `user@host`, `ssh://user@host:22` and `host` all name one host.
fn destination_host(token: &str) -> Option<&str> {
    let token = token.trim_start_matches("ssh://");
    let host = token.rsplit('@').next()?;
    let host = host.split(':').next()?;
    (!host.is_empty()).then_some(host)
}

/// How well an ssh destination names the machine `alias`, best-first, or `None`
/// when the two are unrelated.
///
/// The two names come from different places and often disagree in shape: a
/// producer reports its hostname (`arrhenius1`) while the human types an
/// `~/.ssh/config` Host (`Arrhenius`). `hosts` resolves the latter to its
/// `HostName` so a config that renames a machine entirely still matches.
pub fn host_score(destination: &str, alias: &str, hosts: &SshHosts) -> Option<u8> {
    let direct = name_score(destination, alias);
    let configured = hosts
        .host_name(destination)
        .and_then(|host_name| name_score(host_name, alias));
    direct.max(configured)
}

/// How well one hostname answers to `alias`: exact, first label, then prefix.
fn name_score(host: &str, alias: &str) -> Option<u8> {
    let host = host.trim();
    let alias = alias.trim();
    if host.is_empty() || alias.is_empty() {
        return None;
    }
    if host.eq_ignore_ascii_case(alias) {
        return Some(3);
    }
    let label = host.split('.').next().unwrap_or_default();
    if label.eq_ignore_ascii_case(alias) {
        return Some(2);
    }
    // `Arrhenius` is how a login node called `arrhenius1` is usually spelled.
    shares_prefix(label, alias).then_some(1)
}

/// Whether two names agree over the whole of the shorter one.
///
/// Compared by character rather than by byte: an alias is whatever the machine
/// is called, and slicing a multi-byte name in the middle would panic a
/// surface that is only trying to label a row.
fn shares_prefix(left: &str, right: &str) -> bool {
    let overlap = left.chars().count().min(right.chars().count());
    if overlap < MIN_PREFIX {
        return false;
    }
    left.chars()
        .zip(right.chars())
        .take(overlap)
        .all(|(left, right)| left.eq_ignore_ascii_case(&right))
}

/// `Host` → `HostName` from the user's `~/.ssh/config`.
///
/// The same file Settings reads its Machines list from (CLAUDE.md invariant 5),
/// and read the same way: literal Host names only. Patterns, negations and
/// `Include` are skipped rather than half-implemented — an unresolved Host
/// simply falls back to matching on the name the human typed.
#[derive(Debug, Default)]
pub struct SshHosts {
    entries: Vec<(String, String)>,
}

impl SshHosts {
    /// Read `~/.ssh/config` once per process.
    ///
    /// The file does not change while a sidebar is open in any way that would
    /// justify re-parsing it on every remote-job scan.
    pub fn load() -> &'static Self {
        static HOSTS: OnceLock<SshHosts> = OnceLock::new();
        HOSTS.get_or_init(Self::read)
    }

    fn read() -> Self {
        let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
            return Self::default();
        };
        match fs::read_to_string(home.join(".ssh").join("config")) {
            Ok(text) => Self::parse(&text),
            Err(_) => Self::default(),
        }
    }

    pub fn parse(text: &str) -> Self {
        let mut entries: Vec<(String, String)> = Vec::new();
        let mut current: Vec<String> = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((keyword, value)) = split_directive(line) else {
                continue;
            };
            if keyword.eq_ignore_ascii_case("host") {
                current = value
                    .split_whitespace()
                    .filter(|host| !host.contains(['*', '?', '!']))
                    .map(str::to_string)
                    .collect();
            } else if keyword.eq_ignore_ascii_case("hostname") {
                for host in &current {
                    if !entries.iter().any(|(name, _)| name == host) {
                        entries.push((host.clone(), value.to_string()));
                    }
                }
            }
        }
        Self { entries }
    }

    /// The `HostName` configured for `host`, if the config names one.
    pub fn host_name(&self, host: &str) -> Option<&str> {
        let host = host.trim();
        self.entries
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(host))
            .map(|(_, host_name)| host_name.as_str())
    }
}

/// `Host foo` and `Host=foo` are one directive.
fn split_directive(line: &str) -> Option<(&str, &str)> {
    let (keyword, value) = line
        .split_once(|c: char| c.is_whitespace() || c == '=')
        .map(|(keyword, value)| (keyword, value.trim_start_matches(['=', ' ', '\t'])))?;
    let value = value.trim();
    (!keyword.is_empty() && !value.is_empty()).then_some((keyword, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_remote_shell_clients_hold_a_session_elsewhere() {
        assert!(is_remote_client("ssh"));
        assert!(is_remote_client("/usr/bin/ssh"));
        assert!(is_remote_client("mosh-client"));
        assert!(!is_remote_client("zsh"));
        assert!(!is_remote_client("sshfs"));
        assert!(!is_remote_client(""));
    }

    #[test]
    fn the_destination_is_the_first_non_flag_argument() {
        assert_eq!(parse_destination("ssh Arrhenius"), Some("Arrhenius"));
        assert_eq!(
            parse_destination("ssh -p 2222 -o SendEnv=LANG jicli594@arrhenius1"),
            Some("arrhenius1")
        );
        assert_eq!(parse_destination("ssh -4v -p2222 dardel"), Some("dardel"));
        assert_eq!(
            parse_destination("ssh ssh://me@host.example:22"),
            Some("host.example")
        );
    }

    #[test]
    fn a_remote_command_after_the_host_is_not_the_host() {
        assert_eq!(
            parse_destination("ssh arrhenius1 tmux ls"),
            Some("arrhenius1")
        );
    }

    #[test]
    fn tunnels_are_not_sessions_anyone_can_return_to() {
        assert_eq!(
            parse_destination("ssh -n -N -R 17890:127.0.0.1:17890 Arrhenius"),
            None
        );
        assert_eq!(parse_destination("ssh -fN Arrhenius"), None);
    }

    #[test]
    fn lines_that_are_not_a_client_are_skipped() {
        assert_eq!(parse_destination("-zsh"), None);
        assert_eq!(parse_destination(""), None);
        assert_eq!(parse_destination("ssh"), None);
    }

    #[test]
    fn an_exact_name_outranks_a_label_which_outranks_a_prefix() {
        let hosts = SshHosts::default();
        assert_eq!(host_score("arrhenius1", "arrhenius1", &hosts), Some(3));
        assert_eq!(
            host_score("arrhenius1.uu.se", "arrhenius1", &hosts),
            Some(2)
        );
        assert_eq!(host_score("Arrhenius", "arrhenius1", &hosts), Some(1));
    }

    #[test]
    fn a_name_that_is_not_ascii_is_compared_not_sliced() {
        let hosts = SshHosts::default();
        assert_eq!(host_score("研究室-mac", "研究室-mac", &hosts), Some(3));
        assert_eq!(host_score("研究室", "研究室-mac", &hosts), Some(1));
        assert_eq!(host_score("研", "研究室-mac", &hosts), None);
    }

    #[test]
    fn unrelated_hosts_never_match() {
        let hosts = SshHosts::default();
        assert_eq!(host_score("dardel", "arrhenius1", &hosts), None);
        assert_eq!(host_score("a", "arrhenius1", &hosts), None);
        assert_eq!(host_score("", "arrhenius1", &hosts), None);
    }

    #[test]
    fn a_config_that_renames_the_machine_still_matches() {
        let hosts = SshHosts::parse(
            "Host hpc\n  HostName arrhenius1.hpc.example\n  User me\n\nHost *\n  ServerAliveInterval 60\n",
        );
        assert_eq!(hosts.host_name("hpc"), Some("arrhenius1.hpc.example"));
        assert_eq!(hosts.host_name("HPC"), Some("arrhenius1.hpc.example"));
        assert_eq!(host_score("hpc", "arrhenius1", &hosts), Some(2));
    }

    #[test]
    fn the_first_hostname_for_a_host_wins_and_wildcards_are_skipped() {
        let hosts = SshHosts::parse(
            "Host Arrhenius\n    HostName login.hpc.arrhenius.naiss.se\n\n\
             # BEGIN NERVE MANAGED\n\
             Host Arrhenius\n  RemoteForward 17890 127.0.0.1:17890\n\
             Host *.example\n  HostName ignored\n",
        );
        assert_eq!(
            hosts.host_name("Arrhenius"),
            Some("login.hpc.arrhenius.naiss.se")
        );
        assert_eq!(hosts.host_name("*.example"), None);
    }

    #[test]
    fn directives_may_be_written_with_an_equals_sign() {
        let hosts = SshHosts::parse("Host=box\nHostName=box.example\n");
        assert_eq!(hosts.host_name("box"), Some("box.example"));
    }
}
