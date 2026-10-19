use serde::Deserialize;
use serde::de::Deserializer;

/// How the hub is spreading the interrupt channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NotifyPolicy {
    /// Every connected surface that has notifications on may fire.
    ///
    /// Default for a *missing* `notify` object, so a surface talking to an
    /// older hub does not go silent. A present object with a missing or
    /// unknown `policy` is [`Self::Single`] — the hub's own default — so two
    /// surfaces do not both banner.
    All,
    /// Only [`NotifyLease::owner`] may fire.
    #[default]
    Single,
}

/// The lease one SSE frame (or `/v1/health`) carries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NotifyLease {
    pub policy: NotifyPolicy,
    pub owner: Option<String>,
    pub surfaces: Vec<String>,
    pub watchers: usize,
}

impl<'de> Deserialize<'de> for NotifyLease {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            policy: Option<String>,
            #[serde(default)]
            owner: Option<String>,
            #[serde(default)]
            surfaces: Vec<String>,
            #[serde(default)]
            watchers: usize,
        }
        let raw = Raw::deserialize(deserializer)?;
        let policy = match raw
            .policy
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("all") => NotifyPolicy::All,
            Some("single") => NotifyPolicy::Single,
            // Present object, policy missing or not a spelling we know.
            Some(_) | None => NotifyPolicy::Single,
        };
        Ok(Self {
            policy,
            owner: raw.owner,
            surfaces: raw.surfaces,
            watchers: raw.watchers,
        })
    }
}
