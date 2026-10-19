//! Structured facets — the only things a status may be derived from.
//!
//! Ported field for field from `Nerve/Nerve/Models/CoreTypes.swift`. Display
//! derivation (`Job.status`, ribbon colours) is deliberately **not** here: the
//! hub stores facets, surfaces paint them.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::time::WireTime;

/// Declare a wire enum that survives values it has never heard of.
///
/// Producers spell facets freely and hooks must never be blocked by a typo, so
/// decoding lower-cases the input and degrades to `fallback` instead of failing
/// the envelope (`CoreTypes.swift:14`). The variant marked `#[default]` is what
/// a job assumes when the field is absent altogether.
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
            /// The single spelling this facet ever writes to the wire.
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

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.wire())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(deserializer)?;
                Ok(Self::from_wire(&raw))
            }
        }
    };
}

lenient_enum! {
    /// Lifecycle stage only. Activity, attention and failure live elsewhere.
    Lifecycle, fallback = Unknown {
        Created => "created",
        Pending => "pending",
        /// A report that omits its stage is a report about open work (`Job.make`).
        #[default]
        Active => "active",
        Suspended => "suspended",
        Ended => "ended",
        Unknown => "unknown",
    }
}

lenient_enum! {
    /// How badly the job wants a human back in the agent UI.
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
        /// Reported without a recognisable outcome — never inferred by the hub.
        #[default]
        Unknown => "unknown",
    }
}

/// Why the job is asking for a human.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attention {
    pub level: AttentionLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deferrable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<WireTime>,
}

/// What the job is doing now. Not an independent run object.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Current {
    /// Open type string: `thinking`, `tool`, `building`, `custom.foo`, …
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<WireTime>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProgressKind {
    #[default]
    None,
    Indeterminate,
    Determinate,
    Metrics,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressMetric {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub kind: ProgressKind,
    /// Only when the producer has a trustworthy ratio in `0...1`. Never invented.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Vec<ProgressMetric>>,
}

/// Who reported the job (claude-code, codex, demo…). Not a machine.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProducerInfo {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
}

/// Where a surface should send the human back to.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationInfo {
    // Swift spells this property `openURL`; camelCase would give `openUrl`.
    #[serde(rename = "openURL", default, skip_serializing_if = "Option::is_none")]
    pub open_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_path: Option<String>,
}

/// Lifecycle of one declared action. Unknown spellings are rejected here (as in
/// Swift) because an action state is a control value, not free-form telemetry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionState {
    Available,
    Pending,
    Succeeded,
    Failed,
    Expired,
    Unsupported,
}

impl ActionState {
    pub fn wire(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Pending => "pending",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Expired => "expired",
            Self::Unsupported => "unsupported",
        }
    }

    /// Whether a producer may report this as the end of a pending request
    /// (`SubjectStore.swift:979`).
    pub fn is_completion(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Expired)
    }
}

/// An action a producer declares on a job. The hub echoes these verbatim —
/// Open/Copy and friends are derived by surfaces, never here.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobAction {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub state: ActionState,
    pub destructive: bool,
    pub confirmation_required: bool,
}
