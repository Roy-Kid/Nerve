//! One frame from `GET /v1/stream`, and the subset of a job this surface paints.
//!
//! Decoding only. Nothing here derives a status, counts anything or knows what
//! a status line is — a frame is what the hub said, and the modules above
//! decide what it means.
//!
//! The wire shape is the hub's (`crates/nerve-hub/src/model/`), read leniently:
//! a surface renders what it was sent and never refuses a whole frame over one
//! field it does not model. Only `id` is required.

use std::fmt;

use serde::{Deserialize, Deserializer};
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

use crate::notify::NotifyLease;

/// Declare a wire enum that survives values it has never heard of.
///
/// Ported from `crates/nerve-hub/src/model/facet.rs`: input is trimmed and
/// lower-cased, and an unrecognised spelling degrades to `fallback` instead of
/// failing the frame. The variant marked `#[default]` is what an absent field
/// means.
macro_rules! lenient_enum {
    (
        $(#[$meta:meta])*
        $name:ident, fallback = $fallback:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident => $wire:literal
            ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                $variant
            ),+
        }

        impl $name {
            /// The single spelling this facet has on the wire.
            pub fn wire(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }

            fn from_wire(raw: &str) -> Self {
                match raw.trim().to_ascii_lowercase().as_str() {
                    $($wire => Self::$variant,)+
                    _ => Self::$fallback,
                }
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(deserializer)?;
                Ok(Self::from_wire(&raw))
            }
        }

        /// Written back as the same single spelling it is read from, so a
        /// facet that makes a round trip through a surface's settings comes
        /// out identical to what the hub sent.
        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.wire())
            }
        }
    };
}

lenient_enum! {
    /// Lifecycle stage only. Activity, attention and failure live elsewhere.
    Lifecycle, fallback = Unknown {
        Created => "created",
        Pending => "pending",
        /// A report that omits its stage is a report about open work.
        #[default]
        Active => "active",
        Suspended => "suspended",
        Ended => "ended",
        Unknown => "unknown",
    }
}

lenient_enum! {
    /// How badly the job wants a human back in the agent UI.
    ///
    /// Ordered, ascending, in the order declared here (`CoreTypes.swift:29-41`):
    /// the derivation compares with `>=` (`Subject.swift:43`, `:56`), so the
    /// ordering is contract, not convenience.
    #[derive(PartialOrd, Ord)]
    AttentionLevel, fallback = None {
        #[default]
        None => "none",
        Informational => "informational",
        Suggested => "suggested",
        Required => "required",
        Urgent => "urgent",
    }
}

lenient_enum! {
    /// Liveness of the job as reported by its producer.
    Health, fallback = Unknown {
        #[default]
        Ok => "ok",
        Degraded => "degraded",
        Unresponsive => "unresponsive",
        Unknown => "unknown",
    }
}

lenient_enum! {
    /// How the work finished (or, for `partial`, how far it got).
    Outcome, fallback = Unknown {
        Success => "success",
        Failure => "failure",
        Cancelled => "cancelled",
        Partial => "partial",
        /// Reported without a recognisable outcome — never inferred.
        #[default]
        Unknown => "unknown",
    }
}

/// A UTC instant at whole-second precision — the one wire date shape
/// (`crates/nerve-hub/src/model/time.rs`).
///
/// RFC3339 is read (fractions and offsets included) and normalised on the way
/// in, so what a surface compares is what the hub publishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WireTime(OffsetDateTime);

impl WireTime {
    /// The underlying instant, for callers doing calendar arithmetic.
    pub fn instant(self) -> OffsetDateTime {
        self.0
    }
}

impl<'de> Deserialize<'de> for WireTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;

        let raw = String::deserialize(deserializer)?;
        let instant = OffsetDateTime::parse(&raw, &Rfc3339).map_err(D::Error::custom)?;
        let utc = instant.to_offset(UtcOffset::UTC);
        // `0` is always a valid nanosecond, so the fallback never runs; it is
        // here because a surface must not panic on a producer's clock value.
        Ok(Self(utc.replace_nanosecond(0).unwrap_or(utc)))
    }
}

/// What the job is doing now.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Current {
    /// Open producer vocabulary: `thinking`, `tool`, `building`, `custom.foo`, …
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
}

/// Why the job is asking for a human.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attention {
    #[serde(default)]
    pub level: AttentionLevel,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
}

/// Who reported the job (claude-code, codex, demo…). Not a machine.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProducerInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

/// Workspace labels from the hook (`context.workspace`, `context.project`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobContext {
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
}

/// Hook metadata — pid is the agent process when reported.
///
/// Deliberately narrow: a surface that needs more should widen this additively
/// (unknown keys are already ignored). `model` / `agent_type` / `slot` ride on
/// every snapshot from `hook::build` and used to be dropped here, which is why
/// no surface could show them.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobExtensions {
    #[serde(default)]
    pub hub: HubDisplay,
    #[serde(default)]
    pub pid: Option<u32>,
    /// What the human last asked this agent, as the producer reported it and
    /// the hub kept it (`crates/nerve-hub/src/state/store.rs` STICKY_EXTENSIONS).
    #[serde(default)]
    pub last_prompt: Option<String>,
    /// Model the producer reported, e.g. `claude-fable-5-1`.
    #[serde(default)]
    pub model: Option<String>,
    /// Agent type for a subagent-driven turn, e.g. `Explore`.
    #[serde(default)]
    pub agent_type: Option<String>,
    /// UI slot key — a new session in the same slot supersedes this one.
    #[serde(default)]
    pub slot: Option<String>,
    /// Producer role (`group`, `member`, `job`, …). Unknown keys stay ignored.
    #[serde(default)]
    pub role: Option<String>,
    /// Group this member belongs to.
    #[serde(default)]
    pub group_id: Option<String>,
    /// Legacy parent id. Surfaces treat it as [`Self::group_id`] when that is absent.
    #[serde(default)]
    pub parent_job_id: Option<String>,
    /// `false` keeps the row off the ribbon.
    #[serde(default)]
    pub paint_ribbon: Option<bool>,
}

/// Where a surface should send the human back to.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationInfo {
    #[serde(rename = "openURL", default, skip_serializing_if = "Option::is_none")]
    pub open_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_path: Option<String>,
}

/// One hub-maintained observation, embedded in each job.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEntry {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub job_id: Option<String>,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, alias = "timestamp")]
    pub at: Option<WireTime>,
}

/// An action a producer declares. Surfaces fulfill open/copy locally.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobAction {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub kind: String,
}

/// The subset of a hub job this surface paints.
///
/// Everything but `id` defaults to what the hub itself would have filled in
/// (`crates/nerve-hub/src/model/job.rs:26-69`), and every key this struct has
/// never heard of — the hub's own `timeline` included — is ignored.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: String,
    #[serde(default = "JobView::default_kind")]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub alias: String,

    #[serde(default)]
    pub lifecycle: Lifecycle,
    #[serde(default)]
    pub current: Option<Current>,
    #[serde(default)]
    pub attention: Attention,
    #[serde(default)]
    pub health: Health,
    #[serde(default)]
    pub outcome: Option<Outcome>,
    #[serde(default)]
    pub producer: ProducerInfo,

    #[serde(default)]
    pub context: JobContext,

    #[serde(default)]
    pub extensions: JobExtensions,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<LocationInfo>,
    #[serde(default)]
    pub actions: Vec<JobAction>,
    #[serde(default)]
    pub timeline: Vec<TimelineEntry>,

    #[serde(default)]
    pub created_at: Option<WireTime>,
    #[serde(default)]
    pub updated_at: Option<WireTime>,
}

impl JobView {
    /// What a producer means when it reports work without naming a kind.
    fn default_kind() -> String {
        "session".to_string()
    }
}

/// One frame from `GET /v1/stream`.
///
/// `jobs` is the authority; `departed` is a removal hint kept apart so no
/// caller can count it by accident. `notify` is the interrupt lease — absent
/// on an older hub, which decodes as "everyone may fire".
#[derive(Debug, Deserialize)]
pub struct Frame {
    #[serde(default)]
    pub jobs: Vec<JobView>,
    #[serde(rename = "hubInstanceId")]
    pub hub_instance_id: String,
    pub revision: u64,
    #[serde(default)]
    pub machines: Vec<serde_json::Value>,
    pub summary: HubSummary,
    #[serde(default)]
    pub departed: Vec<JobView>,
    #[serde(default)]
    pub notify: NotifyLease,
    #[serde(default)]
    pub notifications: Vec<HubNotification>,
    #[serde(default, rename = "protocolVersion")]
    pub protocol_version: u32,
}

impl Frame {
    /// Read one `data:` payload.
    ///
    /// Text that will not decode is an error rather than an empty frame: the
    /// caller decides whether to keep the last segment or go offline, and
    /// silently painting zero jobs would be a lie either way.
    pub fn decode(raw: &str) -> Result<Self, FrameError> {
        let value: serde_json::Value = serde_json::from_str(raw).map_err(FrameError::new)?;
        if value.get("protocolVersion").and_then(|v| v.as_u64()) != Some(2) {
            return Err(FrameError {
                message: "Upgrade Nerve hub: protocol version 2 required".into(),
            });
        }
        let frame: Self = serde_json::from_value(value).map_err(FrameError::new)?;
        if frame.protocol_version != 2 {
            return Err(FrameError {
                message: "Upgrade Nerve hub: protocol version 2 required".into(),
            });
        }
        Ok(frame)
    }

    /// Read `GET /v1/jobs` — a bare array, not an SSE envelope.
    pub fn decode_jobs_list(raw: &str) -> Result<Vec<JobView>, FrameError> {
        let value: serde_json::Value = serde_json::from_str(raw).map_err(FrameError::new)?;
        if value.as_array().is_some_and(|jobs| {
            jobs.iter().any(|job| {
                !job.get("display")
                    .is_some_and(|display| display.is_object())
            })
        }) {
            return Err(FrameError {
                message: "Upgrade Nerve hub: authoritative task projection required".into(),
            });
        }
        serde_json::from_value(value).map_err(FrameError::new)
    }
}

/// Why a frame could not be read.
#[derive(Debug)]
pub struct FrameError {
    message: String,
}

impl FrameError {
    fn new(error: serde_json::Error) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for FrameError {}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HubDisplay {
    #[serde(default)]
    pub machine_group: String,
    #[serde(default)]
    pub machine_label: String,
    pub status: String,
    pub status_rank: usize,
    pub ask: bool,
    pub busy: bool,
    pub role: String,
    pub group_id: Option<String>,
    pub paints_ribbon: bool,
    #[serde(default)]
    pub order_key: String,
    pub section: String,
    pub status_label: String,
    pub activity: String,
    #[serde(default)]
    pub order: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HubNotification {
    pub id: String,
    pub target: String,
    pub job_id: String,
    pub title: String,
    pub body: String,
    pub level: String,
    pub sound: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HubSummary {
    pub total: usize,
    pub counts: crate::tally::Tally,
    pub ask: usize,
    pub hard_ask: bool,
    pub worst_status: String,
}
impl HubSummary {
    pub fn tally(&self) -> crate::tally::Tally {
        let mut tally = self.counts;
        tally.ask = self.ask;
        tally
    }
}
