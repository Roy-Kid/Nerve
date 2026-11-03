//! The alias the hub reports for *this* machine.
//!
//! Ported from `LocalMachine.alias` (`MachineConfig.swift:92`): the Bonjour
//! LocalHostName when macOS can name one, else the short hostname, else
//! `local` — sanitised to the characters an alias may contain.
//!
//! Resolution is a subprocess, so it happens once, at assembly: the store owns
//! the resulting string and nothing asks the OS again.

use std::process::Command;

/// What a machine is called when neither source answers.
///
/// A label, not a sentinel: any non-empty alias is legal on ingest, and this
/// one is what `fixtures/demo_snapshot.json` already uses.
const FALLBACK_ALIAS: &str = "local";

/// This machine, as the hub names it for jobs that name no other.
pub struct LocalAlias;

impl LocalAlias {
    /// Ask the OS once. Never empty — an empty alias is exactly what ingest
    /// refuses, so it can never be what the hub fills in.
    pub fn resolve() -> String {
        Self::bonjour_local_host_name()
            .or_else(Self::short_hostname)
            .map(|name| Self::sanitize(&name))
            .filter(|alias| !alias.is_empty())
            .unwrap_or_else(|| FALLBACK_ALIAS.to_string())
    }

    /// macOS's stable Bonjour name, which survives DHCP renaming.
    #[cfg(target_os = "macos")]
    fn bonjour_local_host_name() -> Option<String> {
        Self::first_line(Command::new("/usr/sbin/scutil").args(["--get", "LocalHostName"]))
    }

    #[cfg(not(target_os = "macos"))]
    fn bonjour_local_host_name() -> Option<String> {
        None
    }

    /// The hostname up to its first dot.
    ///
    /// Rust's standard library cannot read a hostname, so the hub asks the same
    /// tool a shell would rather than take a dependency for one string.
    fn short_hostname() -> Option<String> {
        let host = Self::first_line(&mut Command::new("hostname"))?;
        host.split('.')
            .next()
            .filter(|short| !short.is_empty())
            .map(str::to_string)
    }

    /// First non-empty line of a command's output, or nothing if it could not
    /// run: an absent tool is a missing answer, never a failure to start.
    fn first_line(command: &mut Command) -> Option<String> {
        let output = command.output().ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8(output.stdout).ok()?;
        let line = text.lines().next()?.trim().to_string();
        (!line.is_empty()).then_some(line)
    }

    /// Alphanumerics plus `.`, `-` and `_` (`MachineConfig.swift:76`).
    fn sanitize(raw: &str) -> String {
        raw.trim()
            .chars()
            .filter(|character| character.is_alphanumeric() || matches!(character, '.' | '-' | '_'))
            .collect()
    }
}

/// What `extensions.terminal.host` says for a job on *this* machine.
///
/// The hub stamps it; the hook's own `gethostname` is not used, because a
/// Mac's kernel name follows whatever network it is on. macOS answers with
/// its Bonjour name (`<LocalHostName>.local`), which does not move; anything
/// else with its whole `hostname`, so login nodes `arrhenius1` and
/// `arrhenius2` stay two machines. Lowercase, as every terminal host is.
pub struct MachineName;

impl MachineName {
    /// Ask the OS once. `None` leaves each job with what its producer said.
    pub fn resolve() -> Option<String> {
        Self::bonjour()
            .or_else(|| LocalAlias::first_line(&mut Command::new("hostname")))
            .map(|name| Self::normalize(&name))
            .filter(|name| !name.is_empty() && !name.contains(char::is_whitespace))
    }

    #[cfg(target_os = "macos")]
    fn bonjour() -> Option<String> {
        LocalAlias::bonjour_local_host_name().map(|name| format!("{name}.local"))
    }

    #[cfg(not(target_os = "macos"))]
    fn bonjour() -> Option<String> {
        None
    }

    fn normalize(raw: &str) -> String {
        raw.trim().to_lowercase()
    }
}

#[cfg(test)]
mod machine_name_tests {
    use super::MachineName;

    #[test]
    fn a_name_is_lowercase_and_whole() {
        assert_eq!(
            MachineName::normalize(" RoydeMacBook-Air.local\n"),
            "roydemacbook-air.local"
        );
        assert_eq!(
            MachineName::normalize("arrhenius1.kemi.uu.se"),
            "arrhenius1.kemi.uu.se"
        );
    }
}
