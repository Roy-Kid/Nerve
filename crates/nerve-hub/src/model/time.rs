//! The one date shape the hub speaks on the wire.
//!
//! Every producer and every surface exchanges ISO8601 **second precision, no
//! fraction, `Z`** — the spelling `crates/nerve-hub/src/hook/` emits
//! and Swift's `.iso8601` strategy accepts. Owning that in a single type keeps
//! the rule out of every struct that carries a date.

use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime, UtcOffset};

/// A UTC instant truncated to whole seconds.
///
/// Truncation happens on construction, not on formatting, so the value the hub
/// compares is exactly the value it publishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WireTime(OffsetDateTime);

impl WireTime {
    /// Adopt an instant from any offset, normalised to UTC whole seconds.
    pub fn new(instant: OffsetDateTime) -> Self {
        let utc = instant.to_offset(UtcOffset::UTC);
        // `0` is always a valid nanosecond, so the fallback never runs; it is
        // here because a state store must not panic on a clock value.
        Self(utc.replace_nanosecond(0).unwrap_or(utc))
    }

    /// The underlying instant, for callers that need calendar arithmetic.
    pub fn instant(self) -> OffsetDateTime {
        self.0
    }

    /// This instant moved forward by `secs` (TTL arithmetic).
    pub fn plus_seconds(self, secs: i64) -> Self {
        Self::new(self.0.saturating_add(Duration::seconds(secs)))
    }

    /// Whole seconds elapsed since `earlier`; negative when `earlier` is later.
    pub fn seconds_since(self, earlier: Self) -> i64 {
        (self.0 - earlier.0).whole_seconds()
    }
}

impl fmt::Display for WireTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            self.0.year(),
            u8::from(self.0.month()),
            self.0.day(),
            self.0.hour(),
            self.0.minute(),
            self.0.second()
        )
    }
}

impl Serialize for WireTime {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for WireTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Read RFC3339 (fractions and non-UTC offsets included) but keep only
        // what the wire format can express: ingest stays fail-open, output stays
        // canonical.
        let raw = String::deserialize(deserializer)?;
        let instant = OffsetDateTime::parse(&raw, &Rfc3339).map_err(D::Error::custom)?;
        Ok(Self::new(instant))
    }
}
