//! The unit of work the whole product is about: one job per conversation.
//!
//! Wire shape and identity rules ported from `Nerve/Nerve/Models/Subject.swift`.
//! The derived display `status` is **not** ported — it belongs to surfaces.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::facet::{
    Attention, ContextInfo, Current, Health, JobAction, Lifecycle, LocationInfo, Outcome,
    ProducerInfo, Progress,
};
use super::time::WireTime;

/// Forward-compatible open fields from producers (`pid`, `slot`, `role`, …).
pub type Extensions = Map<String, Value>;

/// Opaque process-start token stored beside `extensions.pid`.
const PID_START: &str = "pidStart";

/// A unit of work on a machine (session, build, test, deploy, …).
///
/// `id`, `createdAt`, `updatedAt` and `version` are required: every hook sends
/// them, and requiring them keeps envelope decoding honest. The rest default,
/// so a terse producer still yields a well-formed row.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    #[serde(default = "Job::default_kind")]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    /// Machine alias. Empty means "fill in this machine" (`SubjectStore.swift:414`).
    #[serde(default)]
    pub alias: String,

    #[serde(default)]
    pub lifecycle: Lifecycle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<Current>,
    #[serde(default)]
    pub attention: Attention,
    #[serde(default)]
    pub health: Health,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,
    #[serde(default)]
    pub progress: Progress,
    #[serde(default)]
    pub producer: ProducerInfo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<ContextInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<LocationInfo>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub actions: Vec<JobAction>,

    pub created_at: WireTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<WireTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<WireTime>,
    pub updated_at: WireTime,
    /// Millisecond stamp from the producer. Higher wins.
    pub version: u64,

    #[serde(default)]
    pub extensions: Extensions,
}

impl Job {
    /// What a producer means when it reports work without naming a kind.
    pub(crate) fn default_kind() -> String {
        "session".to_string()
    }

    pub fn is_ended(&self) -> bool {
        self.lifecycle == Lifecycle::Ended
    }

    /// Explicit producer role from `extensions.role` (group | member | job | subagent | …).
    pub fn extension_role(&self) -> Option<String> {
        let Some(Value::String(role)) = self.extensions.get("role") else {
            return None;
        };
        let role = role.trim().to_ascii_lowercase();
        (!role.is_empty()).then_some(role)
    }

    /// Legacy agent subagent / noise rows that must not accumulate in the store
    /// (`Subject.swift:127`).
    ///
    /// An explicit `role=group|member|job` (molq rollups) is never noise, even
    /// with a multi-segment id or `paintRibbon=false`.
    pub fn is_legacy_child_noise(&self) -> bool {
        if let Some(role) = self.extension_role() {
            if role == "subagent" {
                return true;
            }
            if matches!(role.as_str(), "group" | "member" | "job") {
                return false;
            }
        }
        if self.extensions.contains_key("parentJobId") {
            return true;
        }
        if self.extensions.get("paintRibbon") == Some(&Value::Bool(false)) {
            return true;
        }
        // `{producer}:{session}` carries one `:`; legacy children carry two or more.
        self.id.matches(':').count() >= 2
    }

    /// Whether this row belongs in the in-memory job map.
    pub fn is_conversation_job(&self) -> bool {
        !self.is_legacy_child_noise()
    }

    /// Whether this row is a leftover child of `parent_id` (`SubjectStore.swift:730`).
    pub fn has_legacy_parent(&self, parent_id: &str) -> bool {
        if let Some(Value::String(parent)) = self.extensions.get("parentJobId")
            && parent == parent_id
        {
            return true;
        }
        // Legacy id shape: `{parentId}:{agentId}`.
        self.id
            .strip_prefix(parent_id)
            .is_some_and(|rest| rest.starts_with(':'))
    }

    /// Producer process id from `extensions.pid`, as number or string.
    ///
    /// `pid <= 1` is never a producer (0 is "no process", 1 is init), so it is
    /// dropped here rather than probed (`SubjectStore.swift:668`).
    pub fn producer_pid(&self) -> Option<i32> {
        let pid = match self.extensions.get("pid")? {
            Value::Number(number) => number
                .as_i64()
                .and_then(|value| i32::try_from(value).ok())
                .or_else(|| number.as_f64().map(|value| value as i32))?,
            Value::String(text) => text.trim().parse::<i32>().ok()?,
            _ => return None,
        };
        (pid > 1).then_some(pid)
    }

    /// Start token recorded beside [`Self::producer_pid`].
    ///
    /// Missing means the OS did not say. That is not proof the process died.
    pub(crate) fn pid_start(&self) -> Option<&str> {
        let text = self.extensions.get(PID_START)?.as_str()?;
        (!text.is_empty()).then_some(text)
    }

    /// Replace the recorded start token. `None` and `""` both clear it.
    pub(crate) fn set_pid_start(&mut self, token: Option<String>) {
        match token {
            Some(token) if !token.is_empty() => {
                self.extensions
                    .insert(PID_START.to_string(), Value::String(token));
            }
            _ => {
                self.extensions.remove(PID_START);
            }
        }
    }
}
