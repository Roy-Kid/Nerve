//! `POST /v1/snapshot` and `POST /v1/events`.
//!
//! Both routes do the same three things: read the body shapes the route
//! accepts, fill the alias at envelope level, and report how much of the
//! payload the store read.

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::model::Envelope;
use crate::state::JobStore;

use super::HubState;

/// What every handler in this module answers with.
type Reply = (StatusCode, Json<Value>);

/// Envelope only: this route has no bare-array
/// fallback, so a bare job array is a decode failure.
pub(super) async fn snapshot(State(state): State<HubState>, body: Bytes) -> Reply {
    match Envelope::decode_snapshot(&body) {
        Ok(envelope) => apply(&state, envelope),
        Err(error) => {
            tracing::debug!(error = %error, "snapshot rejected");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": error.to_string() })),
            )
        }
    }
}

/// Envelope, bare event array, or a single event.
/// Only the envelope form can carry the alias [`fill_alias`] insists on.
pub(super) async fn events(State(state): State<HubState>, body: Bytes) -> Reply {
    match Envelope::decode_events(&body) {
        Ok(envelope) => apply(&state, envelope),
        Err(error) => {
            tracing::debug!(error = %error, "events rejected");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": error.to_string() })),
            )
        }
    }
}

fn apply(state: &HubState, mut envelope: Envelope) -> Reply {
    if let Err(rejection) = fill_alias(&mut envelope) {
        return rejection;
    }
    let alias = envelope.alias.clone();
    let applied = read(&mut state.store(), envelope);
    if applied > 0 {
        // A batch the hub read is a batch surfaces must see. One that read
        // nothing — every event a duplicate, every job stale — leaves the
        // frames alone.
        tracing::info!(applied, alias = alias.as_deref().unwrap_or(""), "snapshot");
        state.changed();
    } else {
        tracing::debug!(
            alias = alias.as_deref().unwrap_or(""),
            "snapshot applied nothing"
        );
    }
    (StatusCode::OK, Json(json!({ "applied": applied })))
}

/// Stage one of the two-stage alias fill.
///
/// The envelope names the machine; when it does not, the first job carrying a
/// non-empty alias lends its own, and every empty job alias is filled from the
/// result. Any non-empty alias is legal — ingest is open, with no allow-list
/// and no renaming. Stage two, the fallback to *this* machine's alias, happens
/// inside the store (`SubjectStore.swift:414`) and only for what is still empty
/// after this pass, which is why an envelope alias always wins over it.
///
/// The guard reads `alias` and `jobs` only, never `events`: an event array has
/// nowhere to carry an alias, so that shape can only ever be refused here.
fn fill_alias(envelope: &mut Envelope) -> Result<(), Reply> {
    let declared = envelope.alias.as_deref().unwrap_or_default().trim();
    // Emptiness is judged before trimming, as in Swift: a job whose alias is
    // pure whitespace still lends it, and lends nothing.
    let borrowed = envelope
        .jobs
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|job| !job.alias.is_empty())
        .map_or("", |job| job.alias.trim());

    let reported = if declared.is_empty() {
        borrowed
    } else {
        declared
    };
    if reported.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "alias required" })),
        ));
    }
    let reported = reported.to_string();

    envelope.alias = Some(reported.clone());
    for job in envelope.jobs.iter_mut().flatten() {
        let trimmed = job.alias.trim();
        job.alias = if trimmed.is_empty() {
            reported.clone()
        } else {
            trimmed.to_string()
        };
    }
    Ok(())
}

/// Jobs first, then events (`SubjectStore.swift:407`).
///
/// Every job counts — an ended or legacy row was still read — while an event
/// counts only when it changed something, so a replayed batch answers
/// `{"applied":0}`.
fn read(store: &mut JobStore, envelope: Envelope) -> usize {
    let mut applied = store.apply_snapshot(envelope.jobs.unwrap_or_default());
    for event in envelope.events.unwrap_or_default() {
        if store.apply_event(event) {
            applied += 1;
        }
    }
    applied
}
