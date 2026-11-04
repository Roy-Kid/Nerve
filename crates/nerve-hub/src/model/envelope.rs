//! The body producers POST to `/v1/snapshot` and `/v1/events`.
//!
//! Machine identity is the `alias` string and nothing else: any non-empty alias
//! is legal (open ingest, no allow-list). `machineKind` is stored and never
//! read — it exists so producers can report a platform without the hub
//! inventing meaning for it.

use serde::Deserialize;

use super::event::Event;
use super::job::Job;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub alias: Option<String>,
    pub machine_kind: Option<String>,
    pub jobs: Option<Vec<Job>>,
    pub events: Option<Vec<Event>>,
}

impl Envelope {
    /// Read a `POST /v1/snapshot` body.
    ///
    /// Envelope or nothing — this route has no array fallback. An empty body is
    /// an empty envelope, which the alias guard then refuses.
    pub fn decode_snapshot(body: &[u8]) -> Result<Self, serde_json::Error> {
        if body.is_empty() {
            return Ok(Self::default());
        }
        serde_json::from_slice(body)
    }

    /// Read a `POST /v1/events` body.
    ///
    /// Three shapes, tried in this order: envelope, bare event array, single
    /// event. The order is load-bearing and lossy — **every** envelope field is
    /// optional, so a single event *object* decodes as an envelope carrying no
    /// events at all and the payload is silently dropped. The port keeps that
    /// quirk deliberately: it is what the running app does, and surfaces pin the
    /// resulting `{"applied":0}`.
    pub fn decode_events(body: &[u8]) -> Result<Self, serde_json::Error> {
        if body.is_empty() {
            return Ok(Self::default());
        }
        let as_envelope = match serde_json::from_slice::<Self>(body) {
            Ok(envelope) => return Ok(envelope),
            Err(error) => error,
        };
        if let Ok(events) = serde_json::from_slice::<Vec<Event>>(body) {
            return Ok(Self {
                events: Some(events),
                ..Self::default()
            });
        }
        if let Ok(event) = serde_json::from_slice::<Event>(body) {
            return Ok(Self {
                events: Some(vec![event]),
                ..Self::default()
            });
        }
        // Nothing fit: report the envelope attempt, the most informative of the
        // three for a producer reading the error.
        Err(as_envelope)
    }
}
