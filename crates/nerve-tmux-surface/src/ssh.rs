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
//! Matching is by name, and the name is the user's own. A local job is matched
//! by identity — the agent's pid sits on exactly one pane's tty. A remote job
//! names the `~/.ssh/config` Host the hub reached its machine through
//! (`extensions.sshHost`); a pane reaches that machine when its `ssh` was
//! pointed at that same Host ([`reaches`]). The file decides where both land,
//! so nothing here guesses from a hostname that a login node or a Mac spells
//! its own way.

/// Commands that mean "this pane is somewhere else".
///
/// `pane_current_command` is the pane's foreground process, which stays `ssh`
/// no matter what runs on the far side — a remote tmux included.
const CLIENTS: [&str; 4] = ["ssh", "autossh", "mosh", "mosh-client"];

/// Short ssh options that consume the next argument, so the token after them is
/// never the destination.
const OPTIONS_WITH_VALUE: &str = "bcDeEFIiJLlmOopQRSWw";

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

/// Whether an ssh destination is the Host `host`.
///
/// ssh matches a destination against its config case-insensitively, after any
/// `user@` ([`parse_destination`] already dropped it). A destination typed as
/// the Host's `HostName` is another stanza's business, not this one's.
pub fn reaches(destination: &str, host: &str) -> bool {
    let (destination, host) = (destination.trim(), host.trim());
    !host.is_empty() && destination.eq_ignore_ascii_case(host)
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
    fn a_session_reaches_the_host_it_was_pointed_at_and_no_other() {
        assert!(reaches("Arrhenius", "Arrhenius"));
        assert!(reaches("arrhenius", "Arrhenius"));
        assert!(!reaches("arrhenius1", "Arrhenius"));
        assert!(!reaches("login.hpc.arrhenius.naiss.se", "Arrhenius"));
        assert!(!reaches("Arrhenius", ""));
    }
}
