//! Event name + structured fields → main-session facets.
//!
//! Status is never inferred from free-text titles. Vocabulary matches
//! the former Python mapper (removed). Host HTTP / `nerve-hub hook` feed this.

use serde_json::Value;

use super::payload::{
    agent_type, background_tasks, event_name, get_object, get_str, get_text, notification_type,
    truncate,
};

pub const PROMPT_MAX: usize = 400;

#[derive(Clone, Debug)]
pub struct Facets {
    pub lifecycle: &'static str,
    pub current_type: &'static str,
    pub current_name: Option<String>,
    pub current_summary: Option<String>,
    pub attention_level: &'static str,
    pub attention_reason: Option<&'static str>,
    pub attention_title: Option<String>,
    pub attention_summary: Option<String>,
    pub health: &'static str,
    pub outcome: Option<&'static str>,
    pub ended: bool,
    pub end_reason: Option<String>,
}

impl Facets {
    fn active(
        current_type: &'static str,
        summary: impl Into<Option<String>>,
        name: impl Into<Option<String>>,
    ) -> Self {
        Self {
            lifecycle: "active",
            current_type,
            current_name: name.into(),
            current_summary: summary.into(),
            attention_level: "none",
            attention_reason: None,
            attention_title: None,
            attention_summary: None,
            health: "ok",
            outcome: None,
            ended: false,
            end_reason: None,
        }
    }
}

pub fn map_event(payload: &Value) -> Option<Facets> {
    let event = event_name(payload);
    match event.as_str() {
        "sessionstart" => Some(Facets::active("starting", Some("Ready".into()), None)),
        "sessionend" => Some(session_end(payload)),
        "userpromptsubmit" => {
            let prompt = get_text(payload, &["prompt", "user_prompt"]).unwrap_or_default();
            let summary = truncate(&prompt, 120);
            Some(Facets::active(
                "thinking",
                Some(if summary.is_empty() {
                    "New prompt".into()
                } else {
                    summary
                }),
                None,
            ))
        }
        "pretooluse" => Some(pre_tool(payload)),
        "posttooluse" => Some(post_tool(payload)),
        "subagentstart" => {
            let name = agent_type(payload)
                .or_else(|| get_text(payload, &["description"]))
                .unwrap_or_else(|| "subagent".into());
            Some(Facets::active(
                "subagent",
                Some(format!("Subagent: {name}")),
                Some(name),
            ))
        }
        "subagentstop" => Some(subagent_stop(payload)),
        "posttoolusefailure" | "stopfailure" => Some(failure(&event, payload)),
        "permissionrequest" | "permissiondenied" => Some(permission(payload)),
        "notification"
            if notification_type(payload) == "idle_prompt"
                && background_tasks(payload).is_empty() =>
        {
            None
        }
        "notification" => Some(notification(payload)),
        "stop" => stop(payload),
        "stopcancelled" | "elicitation" => {
            Some(your_turn("Return to the agent to continue".into()))
        }
        "precompact" => Some(precompact(payload)),
        "postcompact" => Some(postcompact(payload)),
        "cwdchanged" => Some(Facets::active(
            "info",
            Some("Workspace changed".into()),
            None,
        )),
        "elicitationresult" => Some(elicitation_result(payload)),
        "taskcreated" | "taskcompleted" => Some(task_note(&event, payload)),
        "teammateidle" => Some(teammate_idle(payload)),
        // Registered but deliberately silent: a facet here would invent status
        // from events that carry none (CLAUDE.md invariant 3). Listed so the
        // intent is testable rather than implied by the catch-all.
        e if SILENT_NOOP.contains(&e) => None,
        _ => None,
    }
}

/// Events the hosts register that must never paint a facet. Fire-and-drop.
const SILENT_NOOP: &[&str] = &[
    "setup",
    "userpromptexpansion",
    "posttoolbatch",
    "messagedisplay",
    "instructionsloaded",
    "configchange",
    "directoryadded",
    "filechanged",
];

fn elicitation_result(payload: &Value) -> Facets {
    let text = get_text(payload, &["summary", "title"]).unwrap_or_default();
    let summary = if text.is_empty() {
        "Elicitation result".into()
    } else {
        truncate(&text, 120)
    };
    Facets::active("info", Some(summary), None)
}

/// Task list chatter: show the structured title, never classify it.
fn task_note(event: &str, payload: &Value) -> Facets {
    let label = if event == "taskcompleted" {
        "Task completed"
    } else {
        "Task created"
    };
    let name = get_text(payload, &["title", "name"]);
    let text = get_text(payload, &["title", "name", "description"]).unwrap_or_default();
    let summary = if text.is_empty() {
        label.to_string()
    } else {
        format!("{label}: {text}")
    };
    Facets::active("info", Some(summary), name)
}

/// A teammate going idle is not a human Ask.
fn teammate_idle(payload: &Value) -> Facets {
    let keys = ["name", "agent_type", "agentType", "description"];
    let name = get_text(payload, &keys);
    let text = name.clone().unwrap_or_default();
    let summary = if text.is_empty() {
        "Teammate idle".into()
    } else {
        format!("Teammate idle: {text}")
    };
    Facets::active("info", Some(summary), name)
}

fn session_end(payload: &Value) -> Facets {
    let reason = get_text(payload, &["reason", "source"]).unwrap_or_default();
    let summary = if reason.is_empty() {
        "Session ended".into()
    } else {
        format!("Session ended ({reason})")
    };
    let cancelled = [
        "clear",
        "logout",
        "prompt_input_exit",
        "bypass_permissions_disabled",
        "resume",
        "superseded",
        "process_gone",
        "aborted",
        "dismissed",
    ];
    let outcome = if cancelled.iter().any(|r| reason.eq_ignore_ascii_case(r)) {
        "cancelled"
    } else {
        "success"
    };
    Facets {
        lifecycle: "ended",
        current_type: "idle",
        current_name: None,
        current_summary: Some(summary),
        attention_level: "none",
        attention_reason: None,
        attention_title: None,
        attention_summary: None,
        health: "ok",
        outcome: Some(outcome),
        ended: true,
        end_reason: (!reason.is_empty()).then_some(reason),
    }
}

const SUBAGENT_TOOLS: &[&str] = &[
    "spawn_subagent",
    "get_command_or_subagent_output",
    "Task",
    "Agent",
];

fn pre_tool(payload: &Value) -> Facets {
    let tool = get_str(payload, &["tool_name", "toolName"]).unwrap_or("tool");
    if SUBAGENT_TOOLS.contains(&tool) {
        let input = get_object(payload, &["tool_input", "toolInput"]).unwrap_or(&Value::Null);
        let sub = get_str(input, &["subagent_type", "subagentType", "description"]);
        let name = sub.unwrap_or(tool).to_string();
        let summary = if let Some(sub) = sub {
            format!("Using {tool} ({sub})")
        } else {
            format!("Using {tool}")
        };
        return Facets::active("subagent", Some(summary), Some(name));
    }
    Facets::active("tool", Some(format!("Using {tool}")), Some(tool.into()))
}

fn post_tool(payload: &Value) -> Facets {
    let tool = get_str(payload, &["tool_name", "toolName"]).unwrap_or("tool");
    if SUBAGENT_TOOLS.contains(&tool) {
        let response =
            get_object(payload, &["tool_response", "toolResponse"]).unwrap_or(&Value::Null);
        let status = get_str(response, &["status"])
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(
            status.as_str(),
            "async_launched" | "running" | "in_progress"
        ) {
            let input = get_object(payload, &["tool_input", "toolInput"]).unwrap_or(&Value::Null);
            let name = get_str(response, &["description"])
                .or_else(|| get_str(input, &["subagent_type", "subagentType", "description"]))
                .unwrap_or(tool)
                .to_string();
            return Facets::active("subagent", Some(format!("Background: {name}")), Some(name));
        }
    }
    Facets::active("tool", Some(format!("Finished {tool}")), Some(tool.into()))
}

fn subagent_stop(payload: &Value) -> Facets {
    let name = agent_type(payload)
        .or_else(|| get_text(payload, &["tool_name", "toolName", "description"]))
        .unwrap_or_else(|| "subagent".into());
    let bg = background_tasks(payload);
    if !bg.is_empty() {
        return background_work(bg);
    }
    Facets::active(
        "thinking",
        Some(format!("Subagent finished: {name}")),
        Some(name),
    )
}

fn failure(event: &str, payload: &Value) -> Facets {
    let tool = get_str(payload, &["tool_name", "toolName"]).unwrap_or("tool");
    let mut summary = format!("Failed: {tool}");
    if event == "stopfailure"
        && let Some(err) = get_text(payload, &["error"])
    {
        let t = truncate(&err, 120);
        if !t.is_empty() {
            summary = t;
        }
    }
    let title = if event == "stopfailure" {
        "Turn failed".to_string()
    } else {
        format!("{tool} failed")
    };
    Facets {
        lifecycle: "active",
        current_type: "tool",
        current_name: Some(tool.into()),
        current_summary: Some(summary),
        attention_level: "informational",
        attention_reason: Some("failure"),
        attention_title: Some(title),
        attention_summary: None,
        health: "degraded",
        outcome: None,
        ended: false,
        end_reason: None,
    }
}

fn permission(payload: &Value) -> Facets {
    let tool = get_str(payload, &["tool_name", "toolName"]).unwrap_or("tool");
    let input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))
        .cloned()
        .unwrap_or(Value::Object(Default::default()));
    let dumped = serde_json::to_string(&input).unwrap_or_else(|_| "{}".into());
    Facets {
        lifecycle: "active",
        current_type: "waiting",
        current_name: None,
        current_summary: Some(format!("Permission: {tool}")),
        attention_level: "required",
        attention_reason: Some("approval"),
        attention_title: Some("Approval needed in agent".into()),
        attention_summary: Some(truncate(&format!("{tool}: {dumped}"), 140)),
        health: "ok",
        outcome: None,
        ended: false,
        end_reason: None,
    }
}

fn notification(payload: &Value) -> Facets {
    let ntype = notification_type(payload);
    let title = get_text(payload, &["title", "message"]).unwrap_or_default();
    let summary = {
        let t = truncate(&title, 120);
        if t.is_empty() {
            if ntype.is_empty() {
                "Notification".into()
            } else {
                ntype.clone()
            }
        } else {
            t
        }
    };
    let bg = background_tasks(payload);

    if ntype == "permission_prompt" || ntype == "permission" {
        return Facets {
            lifecycle: "active",
            current_type: "waiting",
            current_name: None,
            current_summary: Some(summary.clone()),
            attention_level: "required",
            attention_reason: Some("approval"),
            attention_title: Some("Approval needed in agent".into()),
            attention_summary: Some(if summary.is_empty() {
                "Return to the agent to approve".into()
            } else {
                summary
            }),
            health: "ok",
            outcome: None,
            ended: false,
            end_reason: None,
        };
    }

    if matches!(
        ntype.as_str(),
        "idle_prompt" | "agent_needs_input" | "elicitation_dialog"
    ) {
        if !bg.is_empty() {
            return background_work(bg);
        }
        let attention_summary = if !summary.is_empty() && ntype != "idle_prompt" {
            summary
        } else {
            "Return to the agent to continue".into()
        };
        return your_turn(attention_summary);
    }

    if matches!(
        ntype.as_str(),
        "agent_completed" | "elicitation_complete" | "elicitation_response" | "auth_success"
    ) {
        return Facets::active("info", Some(summary), None);
    }

    if !bg.is_empty() {
        return background_work(bg);
    }
    Facets::active("info", Some(summary), None)
}

fn stop_hook_active(payload: &Value) -> bool {
    match payload
        .get("stopHookActive")
        .or_else(|| payload.get("stop_hook_active"))
    {
        Some(Value::Bool(true)) => true,
        Some(Value::String(s)) if s.eq_ignore_ascii_case("true") => true,
        _ => false,
    }
}

/// `None` = this Stop is not a turn idle (keep the last facets).
fn stop(payload: &Value) -> Option<Facets> {
    // Grok fires an extra observe Stop at session end. That is not "your turn".
    let reason = get_str(payload, &["reason"]).unwrap_or("").trim();
    if matches!(reason, "channel_closed" | "shutdown") {
        return None;
    }
    // A blocking Stop gate retries while the agent is still working.
    // Painting Ask here is the "running but orange" bug.
    if stop_hook_active(payload) {
        return Some(Facets::active("thinking", Some("Continuing".into()), None));
    }
    let bg = background_tasks(payload);
    if !bg.is_empty() {
        return Some(background_work(bg));
    }
    Some(Facets::active(
        "completed",
        Some("Turn complete".into()),
        None,
    ))
}

fn precompact(payload: &Value) -> Facets {
    let bg = background_tasks(payload);
    if !bg.is_empty() {
        return background_work(bg);
    }
    Facets::active("thinking", Some("Compacting context".into()), None)
}

fn postcompact(payload: &Value) -> Facets {
    let bg = background_tasks(payload);
    if !bg.is_empty() {
        return background_work(bg);
    }
    Facets::active(
        "thinking",
        Some("Context compacted — continuing".into()),
        None,
    )
}

fn your_turn(attention_summary: String) -> Facets {
    Facets {
        lifecycle: "active",
        current_type: "idle",
        current_name: None,
        current_summary: Some("Your turn — continue in the agent UI".into()),
        attention_level: "suggested",
        attention_reason: Some("input"),
        attention_title: Some("Your turn in agent".into()),
        attention_summary: Some(attention_summary),
        health: "ok",
        outcome: None,
        ended: false,
        end_reason: None,
    }
}

fn running_bg(summary: String, name: &str) -> Facets {
    Facets::active("subagent", Some(summary), Some(name.into()))
}

fn monitor_wait(summary: String, name: &str) -> Facets {
    let mut f = Facets::active("monitor", Some(summary), Some(name.into()));
    f.outcome = Some("partial");
    f
}

/// Compact kind label for the row: one work kind, or `mixed` when they differ.
fn bg_kind_label(kinds: &[&'static str]) -> &'static str {
    let mut uniq = kinds.to_vec();
    uniq.sort_unstable();
    uniq.dedup();
    match uniq.as_slice() {
        [] => "subagent",
        [only] => match *only {
            // `only` is `&&str`; the arm coerces one deref down to `&str`.
            "monitor" | "shell" => only,
            _ => "subagent",
        },
        _ => "mixed",
    }
}

fn background_work(tasks: &[Value]) -> Facets {
    let n = tasks.len();
    let kinds: Vec<&'static str> = tasks.iter().map(task_work_kind).collect();
    let name = bg_kind_label(&kinds);
    // List every task, not just the first: one summary is all a row has.
    let descs: Vec<String> = tasks
        .iter()
        .filter_map(|t| t.as_object())
        .filter_map(|obj| {
            [
                "description",
                "name",
                "agent_type",
                "agentType",
                "type",
                "kind",
            ]
            .iter()
            .find_map(|k| obj.get(*k).and_then(Value::as_str))
        })
        .filter(|d| !d.is_empty())
        .map(|d| truncate(d, 40))
        .filter(|d| !d.is_empty())
        .collect();
    let mut summary = format!("{n} background task(s)");
    if !descs.is_empty() {
        summary = format!("{summary}: {}", truncate(&descs.join(", "), 120));
    }
    if !kinds.is_empty() && kinds.iter().all(|k| *k == "monitor") {
        return monitor_wait(summary, name);
    }
    running_bg(summary, name)
}

fn task_work_kind(task: &Value) -> &'static str {
    let Value::Object(obj) = task else {
        return "subagent";
    };
    let raw = obj
        .get("type")
        .or_else(|| obj.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    if raw.is_empty() {
        let desc = ["description", "name", "agent_type", "agentType"]
            .iter()
            .find_map(|k| obj.get(*k).and_then(Value::as_str))
            .unwrap_or("")
            .to_ascii_lowercase();
        if desc.contains("monitor") {
            return "monitor";
        }
        if desc.contains("shell") || desc.contains("bash") {
            return "shell";
        }
        return "subagent";
    }
    if raw.contains("monitor") {
        return "monitor";
    }
    if raw == "shell"
        || raw == "bash"
        || raw == "command"
        || raw == "local_shell"
        || raw == "powershell"
        || raw.contains("shell")
    {
        return "shell";
    }
    "subagent"
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn grok_permission_denied_is_an_ask() {
        let f = map_event(&json!({
            "hook_event_name": "PermissionDenied",
            "toolName": "run_terminal_command"
        }))
        .unwrap();
        assert_eq!(f.current_type, "waiting");
        assert_eq!(f.attention_reason, Some("approval"));
    }

    #[test]
    fn session_start_is_ready_not_running() {
        let f = map_event(&json!({"hook_event_name": "SessionStart"})).unwrap();
        assert_eq!(f.current_type, "starting");
        assert_eq!(f.attention_level, "none");
    }

    #[test]
    fn stop_without_background_completes_turn() {
        let f = map_event(&json!({"hookEventName": "Stop"})).unwrap();
        assert_eq!(f.current_type, "completed");
        assert_eq!(f.attention_level, "none");
        assert_eq!(f.lifecycle, "active");
        assert_eq!(f.outcome, None);
    }

    #[test]
    fn stop_while_the_gate_is_retrying_stays_running() {
        let f = map_event(&json!({
            "hook_event_name": "Stop",
            "stopHookActive": true
        }))
        .unwrap();
        assert_eq!(f.current_type, "thinking");
        assert_eq!(f.attention_level, "none");
    }

    #[test]
    fn session_end_observe_stop_does_not_paint_your_turn() {
        assert!(
            map_event(&json!({
                "hook_event_name": "Stop",
                "reason": "channel_closed"
            }))
            .is_none()
        );
    }

    #[test]
    fn stop_with_background_tasks_is_running() {
        let f = map_event(&json!({
            "hook_event_name": "Stop",
            "background_tasks": [{"type": "shell", "description": "npm test"}]
        }))
        .unwrap();
        assert_eq!(f.current_type, "subagent");
        assert_eq!(f.attention_level, "none");
    }

    #[test]
    fn stop_monitor_only_is_partial_success() {
        let f = map_event(&json!({
            "hook_event_name": "Stop",
            "background_tasks": [{"type": "monitor"}]
        }))
        .unwrap();
        assert_eq!(f.current_type, "monitor");
        assert_eq!(f.outcome, Some("partial"));
    }

    #[test]
    fn permission_is_approval_not_type_here() {
        let f = map_event(&json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": {"command": "rm -rf /"}
        }))
        .unwrap();
        assert_eq!(f.attention_reason, Some("approval"));
        assert_eq!(
            f.attention_title.as_deref(),
            Some("Approval needed in agent")
        );
    }

    #[test]
    fn permission_denied_is_the_same_ask() {
        let f = map_event(&json!({
            "hook_event_name": "PermissionDenied",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        }))
        .unwrap();
        assert_eq!(f.current_type, "waiting");
        assert_eq!(f.attention_level, "required");
        assert_eq!(f.attention_reason, Some("approval"));
        assert_eq!(
            f.attention_title.as_deref(),
            Some("Approval needed in agent")
        );
    }

    #[test]
    fn stopcancelled_is_your_turn() {
        let f = map_event(&json!({"hook_event_name": "StopCancelled"})).unwrap();
        assert_eq!(f.current_type, "idle");
        assert_eq!(f.attention_level, "suggested");
        assert_eq!(f.attention_reason, Some("input"));
        assert_eq!(f.attention_title.as_deref(), Some("Your turn in agent"));
    }

    #[test]
    fn postcompact_without_background_keeps_working_on_new_copy() {
        let f = map_event(&json!({"hook_event_name": "PostCompact"})).unwrap();
        assert_eq!(f.current_type, "thinking");
        assert_eq!(
            f.current_summary.as_deref(),
            Some("Context compacted — continuing")
        );
        assert_eq!(f.attention_level, "none");
    }

    #[test]
    fn precompact_without_background_still_says_compacting() {
        let f = map_event(&json!({"hook_event_name": "PreCompact"})).unwrap();
        assert_eq!(f.current_summary.as_deref(), Some("Compacting context"));
    }

    #[test]
    fn posttoolusefailure_title_names_the_tool() {
        let f = map_event(&json!({
            "hook_event_name": "PostToolUseFailure",
            "tool_name": "Bash"
        }))
        .unwrap();
        assert_eq!(f.attention_title.as_deref(), Some("Bash failed"));
        assert_eq!(f.health, "degraded");
    }

    #[test]
    fn stopfailure_title_stays_turn_failed() {
        let f = map_event(&json!({
            "hook_event_name": "StopFailure",
            "error": "boom"
        }))
        .unwrap();
        assert_eq!(f.attention_title.as_deref(), Some("Turn failed"));
        assert_eq!(f.current_summary.as_deref(), Some("boom"));
    }

    #[test]
    fn posttooluse_treats_all_four_subagent_tools_as_background() {
        for tool in [
            "spawn_subagent",
            "get_command_or_subagent_output",
            "Task",
            "Agent",
        ] {
            let f = map_event(&json!({
                "hook_event_name": "PostToolUse",
                "tool_name": tool,
                "tool_response": {"status": "async_launched", "description": "explore"}
            }))
            .unwrap();
            assert_eq!(f.current_type, "subagent", "{tool}");
            assert_eq!(
                f.current_summary.as_deref(),
                Some("Background: explore"),
                "{tool}"
            );
        }
    }

    #[test]
    fn cwdchanged_is_info_not_attention() {
        let f = map_event(&json!({"hook_event_name": "CwdChanged"})).unwrap();
        assert_eq!(f.current_type, "info");
        assert_eq!(f.current_summary.as_deref(), Some("Workspace changed"));
        assert_eq!(f.attention_level, "none");
    }

    #[test]
    fn elicitation_is_your_turn_and_result_is_info() {
        let ask = map_event(&json!({"hook_event_name": "Elicitation"})).unwrap();
        assert_eq!(ask.current_type, "idle");
        assert_eq!(ask.attention_level, "suggested");

        let done = map_event(&json!({
            "hook_event_name": "ElicitationResult",
            "title": "Answered"
        }))
        .unwrap();
        assert_eq!(done.current_type, "info");
        assert_eq!(done.current_summary.as_deref(), Some("Answered"));
        assert_eq!(done.attention_level, "none");
    }

    #[test]
    fn task_and_teammate_events_are_info_never_attention() {
        let created = map_event(&json!({
            "hook_event_name": "TaskCreated",
            "title": "write tests"
        }))
        .unwrap();
        assert_eq!(created.current_type, "info");
        assert_eq!(created.current_name.as_deref(), Some("write tests"));
        assert_eq!(
            created.current_summary.as_deref(),
            Some("Task created: write tests")
        );
        assert_eq!(created.attention_level, "none");

        let idle = map_event(&json!({
            "hook_event_name": "TeammateIdle",
            "agent_type": "Explore"
        }))
        .unwrap();
        assert_eq!(idle.current_type, "info");
        assert_eq!(idle.current_name.as_deref(), Some("Explore"));
        assert_eq!(
            idle.current_summary.as_deref(),
            Some("Teammate idle: Explore")
        );
        assert_eq!(idle.attention_level, "none");
    }

    #[test]
    fn silent_noop_events_never_paint_a_facet() {
        for event in [
            "setup",
            "userpromptexpansion",
            "posttoolbatch",
            "messagedisplay",
            "instructionsloaded",
            "configchange",
            "directoryadded",
            "filechanged",
        ] {
            assert!(
                map_event(&json!({"hook_event_name": event})).is_none(),
                "{event} must stay silent"
            );
        }
    }

    #[test]
    fn idle_prompt_background_prose_preserves_state() {
        let f = map_event(&json!({
            "hook_event_name": "Notification",
            "notification_type": "idle_prompt",
            "title": "1 shell still running"
        }));
        assert!(f.is_none());
    }

    #[test]
    fn idle_prompt_monitor_prose_preserves_state() {
        let f = map_event(&json!({
            "hook_event_name": "Notification",
            "notification_type": "idle_prompt",
            "title": "Waiting for monitor"
        }));
        assert!(f.is_none());
    }

    #[test]
    fn session_end_clear_is_cancelled() {
        let f = map_event(&json!({
            "hook_event_name": "SessionEnd",
            "reason": "clear"
        }))
        .unwrap();
        assert!(f.ended);
        assert_eq!(f.outcome, Some("cancelled"));
    }

    #[test]
    fn precompact_without_background_is_thinking() {
        let f = map_event(&json!({"hook_event_name": "PreCompact"})).unwrap();
        assert_eq!(f.current_type, "thinking");
        assert_eq!(f.current_summary.as_deref(), Some("Compacting context"));
        assert_eq!(f.attention_level, "none");
    }

    #[test]
    fn postcompact_without_background_keeps_working() {
        let f = map_event(&json!({"hook_event_name": "PostCompact"})).unwrap();
        assert_eq!(f.current_type, "thinking");
        assert_eq!(f.attention_level, "none");
    }

    #[test]
    fn postcompact_with_monitor_stays_monitor() {
        let f = map_event(&json!({
            "hook_event_name": "PostCompact",
            "background_tasks": [{"type": "monitor"}]
        }))
        .unwrap();
        assert_eq!(f.current_type, "monitor");
        assert_eq!(f.outcome, Some("partial"));
    }

    #[test]
    fn calm_notification_text_is_not_attention() {
        let f = map_event(&json!({
            "hook_event_name": "Notification",
            "title": "Build failed in CI"
        }))
        .unwrap();
        assert_eq!(f.current_type, "info");
        assert_eq!(f.attention_level, "none");
    }
}
