//! Which machine a job runs on.
//!
//! The hub says. A job it reached through `ssh <Host>` carries that Host as
//! `extensions.sshHost`: a stanza of the user's `~/.ssh/config`, the one file
//! that decides where `ssh` lands, so the same name is also the way there.
//! Every other job is the hub's own machine's, because a hook only ever posts
//! to the hub beside it. Nothing here compares hostnames or aliases: a Mac
//! renames itself with its network, and a login node (`arrhenius1`) is not
//! spelled the way the Host that reaches it (`Arrhenius`) is.
//!
//! A pane-matching decision starts with this question, as the hub's reaper
//! does before it trusts a pid: a foreign pid and a foreign workspace path can
//! both collide with a local pane by accident.

use crate::frame::JobView;

/// The ssh Host that reached this job's machine, or `None` when the job is on
/// the hub's own.
pub fn ssh_host(job: &JobView) -> Option<&str> {
    job.extensions
        .ssh_host
        .as_deref()
        .map(str::trim)
        .filter(|host| !host.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(extensions: serde_json::Value) -> JobView {
        serde_json::from_value(serde_json::json!({
            "id": "claude-code:s1", "alias": "arrhenius1", "extensions": extensions
        }))
        .expect("job fixture")
    }

    #[test]
    fn a_job_the_hub_reached_over_ssh_names_that_host() {
        assert_eq!(
            ssh_host(&job(serde_json::json!({"sshHost": " Arrhenius "}))),
            Some("Arrhenius")
        );
    }

    #[test]
    fn every_other_job_is_the_hubs_own_whatever_its_alias() {
        assert_eq!(ssh_host(&job(serde_json::json!({}))), None);
        assert_eq!(ssh_host(&job(serde_json::json!({"sshHost": ""}))), None);
    }
}
