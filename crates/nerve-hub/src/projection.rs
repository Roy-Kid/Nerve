//! Business projection belongs to the hub, never to a surface.
use crate::model::{AttentionLevel, Health, Job, Lifecycle, Outcome};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Display {
    pub status: &'static str,
    pub status_rank: usize,
    pub ask: bool,
    pub busy: bool,
    pub role: String,
    pub group_id: Option<String>,
    pub paints_ribbon: bool,
    pub section: &'static str,
    pub status_label: String,
    pub activity: String,
    pub order_key: String,
}
fn names(value: &str, set: &[&str]) -> bool {
    set.iter().any(|s| s.eq_ignore_ascii_case(value.trim()))
}
pub fn display(job: &Job) -> Display {
    let kind = job.current.as_ref().map(|c| c.kind.as_str()).unwrap_or("");
    let busy = names(kind, &["subagent", "tool", "thinking", "info"]);
    let reason = job.attention.reason.as_deref().unwrap_or("");
    let ask = names(
        reason,
        &[
            "input",
            "approval",
            "auth",
            "permission",
            "decision",
            "elicitation",
            "review",
        ],
    ) && job.attention.level >= AttentionLevel::Suggested;
    let status = if job.outcome == Some(Outcome::Failure) || job.health == Health::Unresponsive {
        "problem"
    } else if job.lifecycle == Lifecycle::Ended {
        if matches!(job.outcome, Some(Outcome::Success | Outcome::Partial)) {
            "success"
        } else {
            "inactive"
        }
    } else if !(job.lifecycle == Lifecycle::Active && busy)
        && job.attention.level >= AttentionLevel::Informational
        && names(
            reason,
            &[
                "resource",
                "dependency",
                "queue",
                "system",
                "lock",
                "throttle",
                "rate",
                "capacity",
                "failure",
            ],
        )
    {
        "waiting"
    } else if !(job.lifecycle == Lifecycle::Active && busy)
        && job.attention.level >= AttentionLevel::Informational
        && (names(
            reason,
            &[
                "input",
                "approval",
                "auth",
                "permission",
                "decision",
                "elicitation",
                "review",
            ],
        ) || job.attention.level >= AttentionLevel::Suggested)
    {
        "attention"
    } else if matches!(job.lifecycle, Lifecycle::Suspended | Lifecycle::Unknown) {
        "inactive"
    } else if matches!(job.lifecycle, Lifecycle::Pending | Lifecycle::Created) {
        "waiting"
    } else if job.health == Health::Degraded {
        "problem"
    } else if job.lifecycle == Lifecycle::Active && job.outcome == Some(Outcome::Partial) {
        "monitor"
    } else if busy && job.lifecycle == Lifecycle::Active {
        "running"
    } else if names(kind, &["monitor"]) {
        "monitor"
    } else if names(kind, &["completed"]) {
        "success"
    } else if names(kind, &["waiting"]) {
        "waiting"
    } else if names(kind, &["idle", "starting", "booting"]) {
        "inactive"
    } else if job.lifecycle == Lifecycle::Active {
        "running"
    } else {
        "inactive"
    };
    let role = job.extension_role().unwrap_or_else(|| "job".into());
    let group_id = job
        .extensions
        .get("groupId")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let status_label = if status == "attention" {
        match reason {
            "approval" | "permission" | "auth" => "Approval needed",
            "input" | "elicitation" => "Input needed",
            "review" => "Review needed",
            "decision" => "Decision needed",
            _ => "Needs attention",
        }
    } else if status == "success" && job.lifecycle == Lifecycle::Active {
        "Turn complete"
    } else {
        match status {
            "problem" => "Problem",
            "waiting" => "Waiting",
            "running" => "Running",
            "monitor" => "Monitor",
            "success" => "Completed",
            _ => "Ready / idle",
        }
    };
    let activity = if !busy && job.attention.level >= AttentionLevel::Suggested {
        job.attention.title.as_deref()
    } else {
        None
    }
    .or_else(|| {
        job.current
            .as_ref()
            .and_then(|c| c.summary.as_deref().or(c.name.as_deref()))
    })
    .unwrap_or("")
    .to_owned();
    let status_rank = [
        "problem",
        "attention",
        "waiting",
        "running",
        "monitor",
        "success",
        "inactive",
    ]
    .iter()
    .position(|s| *s == status)
    .unwrap_or(6);
    let attention_rank = match job.attention.level {
        AttentionLevel::None => 0,
        AttentionLevel::Informational => 1,
        AttentionLevel::Suggested => 2,
        AttentionLevel::Required => 3,
        AttentionLevel::Urgent => 4,
    };
    let health_rank = match job.health {
        Health::Unresponsive => 0,
        Health::Degraded => 1,
        Health::Unknown => 2,
        Health::Ok => 3,
    };
    let order_key = format!(
        "{status_rank}:{}:{health_rank}:{}:{:020}:{}",
        4 - attention_rank,
        if job.outcome == Some(Outcome::Failure) {
            0
        } else {
            1
        },
        i128::from(i64::MAX) - i128::from(job.created_at.instant().unix_timestamp()),
        job.id
    );
    Display {
        order_key,
        status_label: status_label.into(),
        activity,
        status,
        status_rank: [
            "problem",
            "attention",
            "waiting",
            "running",
            "monitor",
            "success",
            "inactive",
        ]
        .iter()
        .position(|s| *s == status)
        .unwrap_or(6),
        ask,
        busy,
        paints_ribbon: role != "member"
            && job.extensions.get("paintRibbon").and_then(|v| v.as_bool()) != Some(false),
        role,
        group_id,
        section: match status {
            "attention" | "problem" => "attention",
            "success" => "recent",
            _ => "active",
        },
    }
}

/// Offline fixture projection used by surface tests and documentation examples.
/// Producers do not get to publish their own business projection.
pub fn project_json(value: serde_json::Value) -> serde_json::Value {
    let mut input = value.clone();
    if input["createdAt"].is_null() {
        input["createdAt"] = input["updatedAt"]
            .as_str()
            .map(|s| serde_json::json!(s))
            .unwrap_or_else(|| serde_json::json!("2026-07-19T12:00:00Z"));
    }
    if input["updatedAt"].is_null() {
        input["updatedAt"] = input["createdAt"].clone();
    }
    if input["version"].is_null() {
        input["version"] = serde_json::json!(1);
    }
    if let Some(actions) = input
        .get_mut("actions")
        .and_then(serde_json::Value::as_array_mut)
    {
        for action in actions {
            for (key, val) in [
                ("destructive", serde_json::json!(false)),
                ("confirmationRequired", serde_json::json!(false)),
                ("state", serde_json::json!("available")),
            ] {
                if action[key].is_null() {
                    action[key] = val;
                }
            }
        }
    }
    let job: Job = serde_json::from_value(input).expect("valid fixture job");
    let mut result = value;
    result["display"] = serde_json::to_value(display(&job)).expect("projection");
    result["display"]["machineLabel"] = result
        .get("alias")
        .cloned()
        .unwrap_or(serde_json::json!("unknown"));
    result["display"]["machineGroup"] = result
        .get("machineId")
        .cloned()
        .or_else(|| result.get("alias").cloned())
        .unwrap_or(serde_json::json!("unknown"));
    result["extensions"]["hub"] = result["display"].clone();
    result
}

/// Counts and group membership are computed once by the authority.
pub fn summarize(jobs: &serde_json::Value) -> serde_json::Value {
    use serde_json::{Value, json};
    let rows = jobs.as_array().map(Vec::as_slice).unwrap_or(&[]);
    let mut counts = serde_json::Map::new();
    for status in [
        "problem",
        "attention",
        "waiting",
        "running",
        "monitor",
        "success",
        "inactive",
    ] {
        counts.insert(status.into(), json!(0));
    }
    let mut ask = 0;
    let mut hard = false;
    let mut groups = std::collections::BTreeMap::<String, Vec<Value>>::new();
    for job in rows {
        if let Some(status) = job["display"]["status"].as_str() {
            let n = counts.get(status).and_then(Value::as_u64).unwrap_or(0);
            counts.insert(status.into(), json!(n + 1));
        }
        if job["display"]["ask"] == true {
            ask += 1;
            hard |= matches!(
                job["attention"]["level"].as_str(),
                Some("required" | "urgent")
            );
        }
        groups
            .entry(job["machineId"].as_str().unwrap_or("").into())
            .or_default()
            .push(job["id"].clone());
    }
    let worst = rows
        .iter()
        .min_by_key(|j| j["display"]["statusRank"].as_u64().unwrap_or(6))
        .map(|j| j["display"]["status"].clone())
        .unwrap_or(json!("inactive"));
    json!({"total":rows.len(),"counts":counts,"ask":ask,"hardAsk":hard,"worstStatus":worst,"machineGroups":groups})
}

/// Exactly one mark per terminal, selected in the hub's authoritative job order.
/// Offline source snapshots cannot advertise a live terminal mark.
pub fn tab_indicators(jobs: &serde_json::Value) -> serde_json::Value {
    use serde_json::json;
    let mut seen = std::collections::HashSet::new();
    let mut indicators = Vec::new();
    for job in jobs.as_array().into_iter().flatten() {
        if job["display"]["sourceOnline"] == false {
            continue;
        }
        let target = &job["extensions"]["terminal"];
        let (Some(host), Some(terminal)) = (target["host"].as_str(), target["terminal"].as_str())
        else {
            continue;
        };
        if host.is_empty()
            || host.len() > 255
            || host.chars().any(char::is_whitespace)
            || host != host.to_lowercase()
            || !terminal.starts_with("/dev/")
            || terminal.len() > 256
        {
            continue;
        }
        if !seen.insert((host, terminal)) {
            continue;
        }
        indicators.push(json!({"target":target,"status":job["display"]["status"],
            "label":format!("{} · {}", job["name"].as_str().unwrap_or("Task"),
                job["display"]["statusLabel"].as_str().unwrap_or("Task")).chars().take(256).collect::<String>()}));
        if indicators.len() == 512 {
            break;
        }
    }
    json!(indicators)
}

#[cfg(test)]
mod terminal_indicator_tests {
    use super::tab_indicators;
    use serde_json::json;

    #[test]
    fn terminal_marks_follow_authoritative_order_and_exclude_offline_sources() {
        let target = json!({"host":"studio","terminal":"/dev/ttys001"});
        let jobs = json!([
            {"name":"Approve","extensions":{"terminal":target},"display":{"status":"attention","statusLabel":"Needs you","sourceOnline":true}},
            {"name":"Build","extensions":{"terminal":target},"display":{"status":"running","statusLabel":"Running","sourceOnline":true}},
            {"name":"Remote","extensions":{"terminal":{"host":"gpu","terminal":"/dev/pts/1"}},"display":{"status":"running","sourceOnline":false}}
        ]);
        let marks = tab_indicators(&jobs);
        assert_eq!(marks.as_array().unwrap().len(), 1);
        assert_eq!(marks[0]["target"], target);
        assert_eq!(marks[0]["status"], "attention");
        assert_eq!(tab_indicators(&json!([])), json!([]));
    }
}
