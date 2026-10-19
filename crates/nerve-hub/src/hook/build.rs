//! Host event + facets → one Nerve job snapshot.

use nerve_platform::path;
use serde_json::{Map, Value, json};

use crate::clock::Clock;
use crate::model::{
    ActionState, Attention, AttentionLevel, ContextInfo, Current, Health, Job, JobAction,
    Lifecycle, LocationInfo, Outcome, ProducerInfo, Progress, ProgressKind, WireTime,
};

use super::Producer;
use super::map::{Facets, PROMPT_MAX};
use super::payload::{
    agent_id, agent_type, cwd, event_name, get_str, get_text, project_name, truncate,
};

pub fn build_job(
    payload: &Value,
    producer: Producer,
    clock: &dyn Clock,
    facets: &Facets,
    session: &str,
) -> Job {
    let now = WireTime::new(clock.now());
    let cwd = cwd(payload);
    let project = project_name(&cwd).to_string();
    let location = location(payload, producer, &cwd, &project);
    let mut extensions = Map::new();
    if let Some(ev) = get_str(payload, &["hook_event_name", "hookEventName", "event"]) {
        extensions.insert("hookEvent".into(), json!(ev));
    }
    extensions.insert("sessionId".into(), json!(session));
    if let (Some(host), Some(terminal)) = (
        get_str(payload, &["terminalHost"]),
        get_str(payload, &["terminalName"]),
    ) && !host.is_empty()
        && host.len() <= 255
        && !host.chars().any(char::is_whitespace)
        && terminal.starts_with("/dev/")
        && terminal.len() <= 256
    {
        extensions.insert(
            "terminal".into(),
            json!({"host":host.to_lowercase(),"terminal":terminal}),
        );
    }
    if let Some(model) = get_text(payload, &["model"]) {
        extensions.insert("model".into(), json!(model));
    }
    if let Some(at) = agent_type(payload) {
        extensions.insert("agentType".into(), json!(at));
    }
    // UI slot + agent PID so surfaces can supersede ghosts and reap dead locals.
    // `grok-post.js` computes both with the same algorithm as `nerve.js` and
    // injects them; the hub falls back to a workspace slot when absent.
    for key in ["agentPid", "agent_pid", "pid"] {
        if let Some(pid) = payload.get(key).and_then(Value::as_u64) {
            extensions.insert("pid".into(), json!(pid));
            break;
        }
    }
    let slot = resolved_slot(producer, payload);
    extensions.insert("slot".into(), json!(slot));
    let event = event_name(payload);
    if (event == "subagentstart" || event == "subagentstop")
        && let Some(aid) = agent_id(payload)
    {
        extensions.insert("agentId".into(), json!(aid));
    }
    if let Some(reason) = &facets.end_reason {
        extensions.insert("endReason".into(), json!(reason));
    }
    if event == "userpromptsubmit"
        && let Some(prompt) = get_text(payload, &["prompt", "user_prompt"])
    {
        let prompt = truncate(&prompt, PROMPT_MAX);
        if !prompt.is_empty() {
            extensions.insert("lastPrompt".into(), json!(prompt));
            extensions.insert(
                "lastPromptAt".into(),
                serde_json::to_value(now).unwrap_or(Value::Null),
            );
        }
    }

    let current = Current {
        kind: facets.current_type.to_string(),
        name: facets.current_name.clone(),
        summary: facets.current_summary.clone(),
        detail: None,
        started_at: if facets.ended { None } else { Some(now) },
    };
    let actions = local_actions(&location);

    Job {
        id: format!("{}:{session}", producer.id()),
        kind: "session".into(),
        name: project.clone(),
        alias: String::new(), // store fills this machine's alias
        lifecycle: if facets.ended {
            Lifecycle::Ended
        } else {
            Lifecycle::Active
        },
        current: Some(current),
        attention: Attention {
            level: match facets.attention_level {
                "informational" => AttentionLevel::Informational,
                "suggested" => AttentionLevel::Suggested,
                "required" => AttentionLevel::Required,
                "urgent" => AttentionLevel::Urgent,
                _ => AttentionLevel::None,
            },
            reason: facets.attention_reason.map(str::to_string),
            title: facets.attention_title.clone(),
            summary: facets.attention_summary.clone(),
            deferrable: None,
            deadline: None,
        },
        health: match facets.health {
            "degraded" => Health::Degraded,
            "unresponsive" => Health::Unresponsive,
            _ => Health::Ok,
        },
        outcome: facets.outcome.map(|o| match o {
            "success" => Outcome::Success,
            "cancelled" => Outcome::Cancelled,
            "partial" => Outcome::Partial,
            "failure" => Outcome::Failure,
            _ => Outcome::Unknown,
        }),
        progress: Progress {
            kind: ProgressKind::None,
            ratio: None,
            label: None,
            metrics: None,
        },
        producer: ProducerInfo {
            id: producer.id().into(),
            name: Some(producer.name().into()),
            kind: Some(producer.kind().into()),
        },
        context: Some(ContextInfo {
            project: Some(project),
            workspace: (!cwd.is_empty()).then_some(cwd),
            labels: Some(vec![producer.key().into(), "session".into()]),
        }),
        location: Some(location),
        capabilities: Vec::new(),
        actions,
        created_at: now,
        started_at: Some(now),
        ended_at: facets.ended.then_some(now),
        updated_at: now,
        version: (clock.now().unix_timestamp_nanos() / 1_000_000) as u64,
        extensions,
    }
}

fn location(payload: &Value, producer: Producer, cwd: &str, project: &str) -> LocationInfo {
    let host = "session";
    let mut focus_hint = format!("{} · {project} · {host}", producer.name());
    if !cwd.is_empty() {
        focus_hint = format!("{focus_hint} · {cwd}");
    }
    let open_url = path::to_file_uri(cwd);
    LocationInfo {
        open_url,
        focus_hint: Some(focus_hint),
        log_path: get_text(payload, &["transcript_path", "transcriptPath"]),
    }
}

/// Display-only actions: Open/Focus first, then Copy, then Open logs.
/// Never approve/submit — Nerve does not reverse-control (invariant 6).
pub(crate) fn local_actions(location: &LocationInfo) -> Vec<JobAction> {
    let has_url = location.open_url.as_deref().is_some_and(|u| !u.is_empty());
    let has_hint = location
        .focus_hint
        .as_deref()
        .is_some_and(|h| !h.is_empty());
    let mut actions = Vec::new();
    if has_url || has_hint {
        let (title, kind) = if has_url {
            ("Open", "open")
        } else {
            ("Focus", "focus")
        };
        actions.push(JobAction {
            id: "open".into(),
            title: title.into(),
            kind: kind.into(),
            state: ActionState::Available,
            destructive: false,
            confirmation_required: false,
        });
    }
    actions.push(JobAction {
        id: "copy".into(),
        title: "Copy".into(),
        kind: "copy_summary".into(),
        state: ActionState::Available,
        destructive: false,
        confirmation_required: false,
    });
    // macOS ActionService already reveals `location.logPath` on this action id.
    if location.log_path.as_deref().is_some_and(|p| !p.is_empty()) {
        actions.push(JobAction {
            id: "open_logs".into(),
            title: "Open logs".into(),
            kind: "open_logs".into(),
            state: ActionState::Available,
            destructive: false,
            confirmation_required: false,
        });
    }
    actions
}

/// Slot the hub indexes. An injected `slotId` wins; otherwise producer + workspace.
///
/// Hooks hash the terminal (`WT_SESSION`, `ConEmuPID`, …) into `slotId`. The
/// cwd fallback is only for a producer that sent none, so two sessions in one
/// workspace do not collapse onto each other.
pub fn resolved_slot(producer: Producer, payload: &Value) -> String {
    get_text(payload, &["slotId", "slot"]).unwrap_or_else(|| slot_id(producer, payload))
}

/// Slot key when the payload names no terminal: producer + workspace.
pub fn slot_id(producer: Producer, payload: &Value) -> String {
    let cwd = cwd(payload);
    format!("{}:{cwd}", producer.key())
}

pub fn ended_supersede(
    payload: &Value,
    producer: Producer,
    previous: &str,
    clock: &dyn Clock,
) -> Job {
    let facets = Facets {
        lifecycle: "ended",
        current_type: "idle",
        current_name: None,
        current_summary: Some("Session ended (superseded)".into()),
        attention_level: "none",
        attention_reason: None,
        attention_title: None,
        attention_summary: None,
        health: "ok",
        outcome: Some("cancelled"),
        ended: true,
        end_reason: Some("superseded".into()),
    };
    let mut job = build_job(payload, producer, clock, &facets, previous);
    job.extensions
        .insert("endReason".into(), json!("superseded"));
    job
}

#[cfg(test)]
mod terminal_identity_tests {
    use super::*;
    #[test]
    fn raw_terminal_identity_survives_hook_projection() {
        let clock = crate::clock::SystemClock;
        let payload = json!({"hook_event_name":"UserPromptSubmit","session_id":"s",
            "terminalHost":"Studio","terminalName":"/dev/ttys001"});
        let facets = super::super::map_event(&payload).unwrap();
        let job = build_job(&payload, Producer::Claude, &clock, &facets, "s");
        assert_eq!(
            job.extensions["terminal"],
            json!({"host":"studio","terminal":"/dev/ttys001"})
        );
    }
}
