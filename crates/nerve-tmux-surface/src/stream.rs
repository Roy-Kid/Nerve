//! Painting one attach into the tmux status line.
//!
//! The attach itself — opening it, reading frames, backing off — is
//! [`nerve_surface_core::stream`]. What is left here is what only a tmux
//! surface does with it: render a [`SummaryRenderer`] template into a
//! `SegmentWriter`, and notice when the tmux server has gone away.
//!
//! [`StreamSession`] never sleeps; the composition root waits. That is what
//! lets a whole reconnect story be a unit test.
//!
//! ─────────────────────────────────────────────────────────────────────────
//! When the offline placeholder is written: every attach that ends writes it —
//! a failed `open`, a clean close and a connection that died mid-flight alike.
//! In all three there is no hub behind the numbers any more, and a status line
//! that keeps painting stale counts lies. [`Attempt::ServerGone`] writes
//! nothing: there is no tmux left to write to.
//! ─────────────────────────────────────────────────────────────────────────

// Re-exported while the extraction lands, so `crate::stream::Backoff` and
// `nerve_tmux_surface::stream::FrameSource` keep resolving in `store.rs` and
// `tests/stream.rs`. Removed with the rest of the facade.
pub use nerve_surface_core::stream::{
    Backoff, FrameSource, HubFrameSource, JOBS_PATH, StreamError,
};

use crate::summary::SummaryRenderer;
use crate::tmux::SegmentWriter;

/// The one stream this surface opens.
pub const STREAM_PATH: &str = "/v1/stream?surface=tmux";

/// How one attach ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attempt {
    /// The attach is over after delivering `frames` frames. Zero means the hub
    /// was never reachable.
    Ended { frames: usize },
    /// tmux itself is gone: the caller exits 0 and drops the connection, which
    /// is how the hub learns this surface left.
    ServerGone,
}

/// One attach, from open to close, painting every frame it is given.
pub struct StreamSession<S: FrameSource, W: SegmentWriter> {
    source: S,
    writer: W,
    summary: SummaryRenderer,
    offline: String,
}

impl<S: FrameSource, W: SegmentWriter> StreamSession<S, W> {
    pub fn new(source: S, writer: W, summary: SummaryRenderer, offline: impl Into<String>) -> Self {
        Self {
            source,
            writer,
            summary,
            offline: offline.into(),
        }
    }

    /// One attach, start to finish. Never sleeps.
    pub fn attach(&mut self) -> Attempt {
        if self.source.open().is_err() {
            return self.go_offline(0);
        }

        let mut frames = 0;
        loop {
            match self.source.next_frame() {
                Ok(Some(frame)) => {
                    frames += 1;
                    let text = self.summary.render(&frame.summary.tally());
                    if self.paint(&text) {
                        return Attempt::ServerGone;
                    }
                }
                // A closed stream and a broken one leave the same hole: there
                // is no hub behind the numbers on screen any more.
                Ok(None) | Err(_) => return self.go_offline(frames),
            }
        }
    }

    /// Write the placeholder and report the attach as over.
    fn go_offline(&mut self, frames: usize) -> Attempt {
        let offline = self.offline.clone();
        if self.paint(&offline) {
            return Attempt::ServerGone;
        }
        Attempt::Ended { frames }
    }

    /// Paint one text; `true` when tmux itself has gone away.
    ///
    /// Any other tmux failure costs this repaint and nothing more: the surface
    /// stays fail-open (CLAUDE.md invariant 2).
    fn paint(&mut self, text: &str) -> bool {
        matches!(self.writer.write_segment(text), Err(err) if err.is_server_gone())
    }
}
