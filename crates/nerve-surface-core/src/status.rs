//! Read hub-projected status. Colours and display labels remain on surfaces.
use crate::frame::JobView;

/// Derived display status.
///
/// Variants are declared in priority order, so the derived `Ord` *is* the
/// priority (`CoreTypes.swift:244-267`):
/// problem > attention > waiting > running > monitor > success > inactive.
///
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StatusClass {
    Problem,
    Attention,
    Waiting,
    Running,
    Monitor,
    Success,
    Inactive,
}

impl StatusClass {
    /// Every class, most urgent first.
    pub const ALL: [StatusClass; 7] = [
        Self::Problem,
        Self::Attention,
        Self::Waiting,
        Self::Running,
        Self::Monitor,
        Self::Success,
        Self::Inactive,
    ];

    /// Lower-case spelling, equal to Swift's `Status.rawValue`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Problem => "problem",
            Self::Attention => "attention",
            Self::Waiting => "waiting",
            Self::Running => "running",
            Self::Monitor => "monitor",
            Self::Success => "success",
            Self::Inactive => "inactive",
        }
    }

    /// Words a person reads. Same table as macOS `Status.title`.
    ///
    /// `label` stays the wire id. A section heading uses this, so Windows does
    /// not title a bucket `ATTENTION`.
    pub fn title(self) -> &'static str {
        match self {
            Self::Problem => "Problem",
            Self::Attention => "Needs you",
            Self::Waiting => "Waiting",
            Self::Running => "Running",
            Self::Monitor => "Monitor",
            Self::Success => "Completed",
            Self::Inactive => "Ready / idle",
        }
    }

    /// The header "running" count is this class alone. Monitor is its own count.
    pub fn counts_as_running(self) -> bool {
        matches!(self, Self::Running)
    }

    /// The header "needs you" count is Attention alone.
    ///
    /// Waiting stays gray. Problem is its own count.
    pub fn counts_as_needs_you(self) -> bool {
        matches!(self, Self::Attention)
    }

    /// The one derivation, ported line for line from `Subject.swift:38-100`.
    ///
    /// The `:NN` comments are that file's line numbers. The three helpers below
    /// are its three `switch`/branch groups — the same grouping `tests/status.rs`
    /// pins section by section — and each answers `None` for exactly the cases
    /// Swift falls through on.
    pub fn of(job: &JobView) -> Self {
        match job.extensions.hub.status.as_str() {
            "problem" => Self::Problem,
            "attention" => Self::Attention,
            "waiting" => Self::Waiting,
            "running" => Self::Running,
            "monitor" => Self::Monitor,
            "success" => Self::Success,
            _ => Self::Inactive,
        }
    }
}
pub fn is_ask_elevated(job: &JobView) -> bool {
    job.extensions.hub.ask
}
