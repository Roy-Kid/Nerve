//! `GET /v1/actions/pending` and `POST /v1/actions/result`.
//!
//! The queue behind these is ported but dormant (spec D6): nothing enqueues
//! today, because Nerve is display-only and never reverse-controls an agent.
//! The routes exist so a producer polling them keeps getting the honest answer
//! it always got — an empty array and a miss — rather than a surprise 404.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::model::ActionState;

use super::HubState;

/// What every handler in this module answers with.
type Reply = (StatusCode, Json<Value>);

/// Which producer is asking. `sourceId` is the legacy spelling; an absent
/// one means "any producer".
///
/// As visible as the handlers that name it in their signature, and no more.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProducerQuery {
    producer_id: Option<String>,
    source_id: Option<String>,
}

impl ProducerQuery {
    fn producer(&self) -> Option<&str> {
        self.producer_id.as_deref().or(self.source_id.as_deref())
    }
}

/// A producer reporting how the action it was handed ended.
#[derive(Debug, Deserialize)]
struct ActionResult {
    id: String,
    state: ActionState,
    #[serde(default)]
    message: Option<String>,
}

/// Expire first, then filter: a request past its TTL
/// must never be handed to a producer.
pub(super) async fn pending(
    State(state): State<HubState>,
    Query(query): Query<ProducerQuery>,
) -> Reply {
    let open = {
        let mut store = state.store();
        store.expire_pending();
        store.pending_json(query.producer())
    };
    (StatusCode::OK, Json(open))
}

/// Record the answer, or say plainly that nothing was waiting for it.
pub(super) async fn result(
    State(state): State<HubState>,
    Query(query): Query<ProducerQuery>,
    body: Bytes,
) -> Reply {
    let report = match serde_json::from_slice::<ActionResult>(&body) {
        Ok(report) => report,
        // Swift echoes its own decode error here; the text is explicitly not
        // part of the contract, only the 400 is.
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": error.to_string() })),
            );
        }
    };

    let completed = state.store().complete_pending(
        &report.id,
        report.state,
        report.message.as_deref(),
        query.producer(),
    );
    if completed {
        // The action's state changed on the job, so the panels showing it must
        // be told. A miss changed nothing.
        state.changed();
        return (StatusCode::OK, Json(json!({ "ok": true })));
    }
    (
        StatusCode::NOT_FOUND,
        // Matched verbatim by `./scripts/nerve.sh --verify-loop`.
        Json(json!({ "error": "pending action not found or producer mismatch" })),
    )
}
