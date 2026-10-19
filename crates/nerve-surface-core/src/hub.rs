//! The loopback hub endpoint.
//!
//! A transport and nothing more — it knows about requests, responses and
//! event-stream framing, and nothing about frames, health or jobs. The modules
//! that speak to the hub ([`crate::launch`] health, [`crate::stream`] SSE, and
//! the popup's one-shot job read) adapt it to their own port, so no module has
//! to own a connection to be unit-testable.
//!
//! HTTP and event-stream framing are a dependency, not something kept here.
//! Both have edge cases that a hand-rolled reader gets wrong quietly: chunked
//! transfer encoding may split a `data:` line across two chunks, headers fold,
//! and a `data:` field may legitimately span several lines. `reqwest` and
//! `eventsource-stream` handle those; this file wires them to the one port.
//!
//! Every method here is blocking to its caller. The runtime is current-thread,
//! private to one [`Hub`], and never reaches a UI loop — a surface's ratatui or
//! egui loop stays exactly as synchronous as it was.
//!
//! The port is fixed at 17890. It is the hub's single-instance lock and is
//! hard-coded by the agent hooks (CLAUDE.md invariant 2), so it is deliberately
//! not configurable here either.

use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use eventsource_stream::{EventStream, Eventsource};
use futures_util::stream::{Stream, StreamExt};
use reqwest::Client;
pub use reqwest::Method as HttpMethod;
use tokio::runtime::{Builder, Runtime};

/// The one ingest port, shared by every producer and every surface.
pub const PORT: u16 = 17890;

/// `http://127.0.0.1:17890`.
pub const BASE_URL: &str = "http://127.0.0.1:17890";

/// Loopback either answers at once or is not there.
const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);

/// A one-shot `GET` that has not finished by now is not going to.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// A stream that has said nothing for this long is dead.
///
/// Applied per read and reset by each one, so a healthy stream lives forever
/// and a half-open socket is noticed within a minute. Comfortably above axum's
/// SSE keep-alive interval (15s), so a quiet hub is never mistaken for a broken
/// one.
const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Why a request did not produce a body.
#[derive(Debug)]
pub enum HubError {
    /// Nothing answered on the loopback port.
    Unreachable(String),
    /// The connection was made and then failed, or answered nonsense.
    Broken(String),
    /// The hub answered, with a status this surface cannot use.
    Status(u16),
}

impl fmt::Display for HubError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreachable(reason) => write!(f, "hub unreachable: {reason}"),
            Self::Broken(reason) => write!(f, "hub connection broken: {reason}"),
            Self::Status(status) => write!(f, "hub answered HTTP {status}"),
        }
    }
}

impl std::error::Error for HubError {}

impl From<reqwest::Error> for HubError {
    fn from(error: reqwest::Error) -> Self {
        // Nothing listening and a connection that died mid-flight are different
        // states to a surface: one paints "offline", the other reconnects.
        if error.is_connect() {
            tracing::debug!(error = %error, "hub unreachable");
            Self::Unreachable(error.to_string())
        } else {
            tracing::debug!(error = %error, "hub broken");
            Self::Broken(error.to_string())
        }
    }
}

/// The client and the runtime that drives it.
struct Transport {
    runtime: Runtime,
    client: Client,
}

impl Transport {
    fn new() -> Option<Self> {
        let runtime = Builder::new_current_thread().enable_all().build().ok()?;
        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            // Per read, reset by each one — an idle timeout, not a budget.
            .read_timeout(STREAM_IDLE_TIMEOUT)
            .build()
            .ok()?;
        Some(Self { runtime, client })
    }
}

/// The hub on this machine's loopback.
///
/// Opening a connection is per-call, which is what makes an unreachable hub a
/// per-call answer rather than a lifetime.
pub struct Hub {
    base: String,
    transport: Option<Arc<Transport>>,
}

impl Hub {
    /// The hub every producer and surface on this machine talks to.
    ///
    /// Infallible on purpose: a surface that could not build a client still has
    /// to run and paint its offline state (fail-open), so the failure surfaces
    /// per request instead of at construction.
    pub fn loopback() -> Self {
        Self {
            base: BASE_URL.to_string(),
            transport: Transport::new().map(Arc::new),
        }
    }

    /// The same transport pointed somewhere else.
    ///
    /// Only for tests: production is always [`Hub::loopback`], because the port
    /// is the hub's single-instance lock and is hard-coded by the agent hooks
    /// (CLAUDE.md invariant 2). It exists so the client and the server can be
    /// exercised against each other on an ephemeral port.
    pub fn at(base: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            transport: Transport::new().map(Arc::new),
        }
    }

    fn transport(&self) -> Result<&Arc<Transport>, HubError> {
        self.transport
            .as_ref()
            .ok_or_else(|| HubError::Broken("no HTTP runtime on this machine".to_string()))
    }

    /// One `GET`, answered as the whole response body.
    pub fn get(&self, path: &str) -> Result<String, HubError> {
        let transport = self.transport()?;
        let url = format!("{}{path}", self.base);
        transport.runtime.block_on(async {
            let response = transport
                .client
                .get(&url)
                .timeout(REQUEST_TIMEOUT)
                .header("Accept", "application/json")
                .send()
                .await?;
            check_status(response.status())?;
            Ok(response.text().await?)
        })
    }

    /// One long-lived `GET`, answered as the events it carries.
    ///
    /// Holding the returned stream open is what keeps the connection — and
    /// therefore the hub's refcount — alive.
    pub fn send_json(
        &self,
        path: &str,
        method: reqwest::Method,
        value: &serde_json::Value,
    ) -> Result<(), HubError> {
        let t = self.transport()?;
        let url = format!("{}{path}", self.base);
        t.runtime.block_on(async {
            let response = t
                .client
                .request(method, url)
                .header("Content-Type", "application/json")
                .body(value.to_string())
                .timeout(REQUEST_TIMEOUT)
                .send()
                .await?;
            if !response.status().is_success() {
                return Err(HubError::Status(response.status().as_u16()));
            }
            Ok(())
        })
    }

    pub fn open(&self, path: &str) -> Result<HubEvents, HubError> {
        let transport = Arc::clone(self.transport()?);
        let url = format!("{}{path}", self.base);
        let events = transport.runtime.block_on(async {
            let response = transport
                .client
                .get(&url)
                .header("Accept", "text/event-stream")
                .send()
                .await?;
            check_status(response.status())?;
            let bytes: BoxedBytes = Box::pin(response.bytes_stream());
            Ok::<_, HubError>(bytes.eventsource())
        })?;

        Ok(HubEvents {
            transport: Arc::clone(&transport),
            events: Box::pin(events),
        })
    }
}

type BoxedBytes = Pin<Box<dyn Stream<Item = reqwest::Result<bytes::Bytes>> + Send>>;

/// One open event stream.
pub struct HubEvents {
    transport: Arc<Transport>,
    events: Pin<Box<EventStream<BoxedBytes>>>,
}

impl HubEvents {
    /// The `data` payload of the next event, or `None` at end of stream.
    ///
    /// Comments, `event:`/`id:`/`retry:` fields and keep-alives are consumed by
    /// the parser and never reach the caller — a surface only ever acts on the
    /// payload.
    pub fn next_payload(&mut self) -> Result<Option<String>, HubError> {
        let transport = Arc::clone(&self.transport);
        transport.runtime.block_on(async {
            match self.events.next().await {
                None => Ok(None),
                Some(Ok(event)) => Ok(Some(event.data)),
                Some(Err(error)) => Err(HubError::Broken(error.to_string())),
            }
        })
    }
}

fn check_status(status: reqwest::StatusCode) -> Result<(), HubError> {
    if status == reqwest::StatusCode::OK {
        Ok(())
    } else {
        Err(HubError::Status(status.as_u16()))
    }
}
