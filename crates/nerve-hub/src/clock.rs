//! Injected time source.
//!
//! Every part of the hub that needs "now" takes a `&dyn Clock` instead of
//! calling the OS, so state-machine tests (version ordering, pending TTL,
//! reaper min-age) stay deterministic.

use std::sync::Mutex;
use std::time::Duration;

use time::OffsetDateTime;

/// Wall-clock reader. Always UTC — the wire format is ISO8601 `Z`.
pub trait Clock: Send + Sync {
    fn now(&self) -> OffsetDateTime;
}

/// The real clock. Used by the binary.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

/// Test clock that only moves when a test moves it.
///
/// Shared by unit and integration tests, hence public rather than
/// `cfg(test)`-only.
pub struct FakeClock {
    now: Mutex<OffsetDateTime>,
}

impl FakeClock {
    pub fn new(start: OffsetDateTime) -> Self {
        Self {
            now: Mutex::new(start),
        }
    }

    /// Move time forward by `step`.
    pub fn advance(&self, step: Duration) {
        let mut now = self.lock();
        *now += step;
    }

    /// Jump to an absolute instant.
    pub fn set(&self, instant: OffsetDateTime) {
        *self.lock() = instant;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, OffsetDateTime> {
        // A poisoned clock means a test already failed elsewhere; recover the
        // value rather than cascading a second panic into unrelated asserts.
        self.now
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Clock for FakeClock {
    fn now(&self) -> OffsetDateTime {
        *self.lock()
    }
}
