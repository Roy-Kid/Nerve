//! The loopback HTTP surface: what producers POST and what surfaces GET.
//!
//! Ported from the app's former `IngestServer.swift`. Handlers decode a body,
//! call the store, and answer `(StatusCode, Json)` — they never panic, never
//! answer 5xx, and never derive display state.
//!
//! Everything about *whether* a request is allowed in (peer, size) and what
//! every answer carries (CORS) lives in [`guard`], so no handler repeats a
//! cross-cutting rule.

mod actions;
mod admin;
mod guard;
mod hook;
mod ingest;
mod routes;
mod stream;

pub use routes::{HubState, router};
pub use stream::{StreamState, stream_router};
