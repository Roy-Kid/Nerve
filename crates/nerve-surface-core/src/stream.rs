//! SSE attachment and transport retry. Disconnect never stops the authority.
use std::time::Duration;

use crate::frame::Frame;
use crate::hub::{Hub, HubError, HubEvents};

/// One-shot job list — same rows a connect frame carries, bare array.
pub const JOBS_PATH: &str = "/v1/jobs";

/// The stream to open, labelled with which surface is asking.
///
/// `?surface=` names this connection for the hub's notify lease. Every
/// surface still sees every job — the label is not a filter.
pub fn stream_path(surface: &str) -> String {
    format!("/v1/stream?surface={surface}")
}

/// Doubling reconnect delay: 0.5s, capped at 30s, reset by any attach that
/// delivered a frame.
#[derive(Clone, Copy, Debug)]
pub struct Backoff {
    next: Duration,
}

impl Backoff {
    /// What the first failure costs.
    pub const FIRST: Duration = Duration::from_millis(500);
    /// However long the hub stays away, never wait longer than this.
    pub const CAP: Duration = Duration::from_secs(30);

    pub fn new() -> Self {
        Self { next: Self::FIRST }
    }

    /// The delay to wait after this failure; doubles for the next one.
    pub fn fail(&mut self) -> Duration {
        let delay = self.next;
        self.next = (delay * 2).min(Self::CAP);
        delay
    }

    /// What the next [`Backoff::fail`] will return, without consuming it.
    pub fn peek(&self) -> Duration {
        self.next
    }

    /// Back to [`Backoff::FIRST`].
    pub fn reset(&mut self) {
        self.next = Self::FIRST;
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new()
    }
}

/// Why a stream is not delivering frames.
#[derive(Debug)]
pub enum StreamError {
    /// Nothing answered on the loopback port.
    Unreachable,
    /// The connection died, or carried something that would not decode.
    Broken(String),
}

/// One long-lived `GET /v1/stream?surface=tmux`.
pub trait FrameSource {
    /// Open the connection.
    fn open(&mut self) -> Result<(), StreamError>;
    /// Block for the next frame. `Ok(None)` means the hub closed the stream.
    fn next_frame(&mut self) -> Result<Option<Frame>, StreamError>;
}

/// The frame source the binary injects: a real SSE connection to the hub.
pub struct HubFrameSource {
    hub: Hub,
    path: String,
    events: Option<HubEvents>,
}

impl HubFrameSource {
    /// `path` is usually [`stream_path`] for the caller's own surface label.
    pub fn new(hub: Hub, path: impl Into<String>) -> Self {
        Self {
            hub,
            path: path.into(),
            events: None,
        }
    }
}

impl FrameSource for HubFrameSource {
    fn open(&mut self) -> Result<(), StreamError> {
        // Dropped first: the old connection has to leave the hub's refcount
        // before the new one joins it, or a reconnect looks like a second
        // surface.
        self.events = None;
        self.events = Some(self.hub.open(&self.path).map_err(StreamError::from)?);
        Ok(())
    }

    fn next_frame(&mut self) -> Result<Option<Frame>, StreamError> {
        let events = self
            .events
            .as_mut()
            .ok_or_else(|| StreamError::Broken("stream was never opened".to_string()))?;

        // Keep-alive comments, `event:`/`id:`/`retry:` fields and the blank
        // separators are the parser's business; only a payload reaches here.
        let Some(payload) = events.next_payload().map_err(StreamError::from)? else {
            return Ok(None);
        };
        Frame::decode(&payload)
            .map(Some)
            .map_err(|err| StreamError::Broken(err.to_string()))
    }
}

impl From<HubError> for StreamError {
    fn from(error: HubError) -> Self {
        match error {
            HubError::Unreachable(_) => Self::Unreachable,
            other => Self::Broken(other.to_string()),
        }
    }
}
