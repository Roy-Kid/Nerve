//! nerve-hub: the cross-platform status hub daemon.
//!
//! This file is the assembly point only — it wires collaborators together and
//! owns no domain logic. State, HTTP handlers, SSE and lifecycle land in their
//! own modules as the spec's tasks fill them in.

pub mod cli;
pub mod clock;
pub mod ensure;
pub mod federation;
pub mod hook;
pub mod http;
pub mod lifecycle;
pub mod log;
pub mod model;
pub mod notification;
pub mod projection;
pub mod remote_focus;
mod runtime;
pub mod sse;
pub mod state;

pub use runtime::HubRuntime;

use std::fmt;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::time::sleep;

use crate::clock::SystemClock;
use crate::state::{JobStore, LocalAlias, SignalProbe};

/// The one address the hub ever listens on.
///
/// The port is not configurable on purpose: binding it is the single-instance
/// lock, and host HTTP hooks POST the same address (`/v1/hook`).
pub const INGEST_ADDR: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 17890));

/// A configured hub, ready to take over the ingest port.
#[derive(Debug, Default)]
pub struct Hub;

impl Hub {
    /// How long a connection still open when the hub decides to stop may hold
    /// the process up.
    ///
    /// Graceful shutdown waits for open connections, and an SSE connection
    /// never ends on its own — a surface that attaches in the instant the timer
    /// fires would otherwise keep a hub alive that has already decided to go.
    const DRAIN: Duration = Duration::from_secs(2);

    pub fn new() -> Self {
        Self
    }

    /// Bind the ingest port and serve until the hub decides to stop.
    ///
    /// Returns [`ServeError::AlreadyRunning`] rather than a generic I/O error
    /// when the port is taken, because that case is not a failure: someone
    /// else already satisfies the caller's post-condition.
    pub async fn serve(self) -> Result<(), ServeError> {
        let listener = match TcpListener::bind(INGEST_ADDR).await {
            Ok(listener) => listener,
            Err(err) if err.kind() == io::ErrorKind::AddrInUse => {
                tracing::info!("already running");
                return Err(ServeError::AlreadyRunning);
            }
            Err(err) => return Err(ServeError::Io(err)),
        };

        tracing::info!(
            addr = %INGEST_ADDR,
            "serving"
        );

        let hub = HubRuntime::new(self.store());
        hub.start_connections();
        let stopping = hub.shutdown();
        let drained = hub.shutdown();
        // `into_make_service_with_connect_info` is what installs the peer
        // address the loopback guard reads; without it every request would be
        // refused as unplaceable.
        let served = axum::serve(
            listener,
            hub.router()
                .into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(stopping);

        tokio::select! {
            result = served => result.map_err(ServeError::Io),
            () = async move { drained.await; sleep(Self::DRAIN).await } => Ok(()),
        }
    }

    /// Wire the collaborators the store needs: the real clock, this machine's
    /// alias, and the liveness probe the reaper consults.
    fn store(&self) -> JobStore {
        JobStore::new(
            Arc::new(SystemClock),
            LocalAlias::resolve(),
            Arc::new(SignalProbe),
        )
    }
}

/// Why [`Hub::serve`] stopped.
#[derive(Debug)]
pub enum ServeError {
    /// The ingest port is held by another hub, which is serving in our place.
    AlreadyRunning,
    /// The listener could not start, or died while serving.
    Io(io::Error),
}

impl fmt::Display for ServeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRunning => {
                write!(
                    f,
                    "a hub already serves http://{INGEST_ADDR}; leaving it alone"
                )
            }
            Self::Io(err) => write!(f, "http://{INGEST_ADDR}: {err}"),
        }
    }
}

impl std::error::Error for ServeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AlreadyRunning => None,
            Self::Io(err) => Some(err),
        }
    }
}
