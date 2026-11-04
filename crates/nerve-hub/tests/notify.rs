//! Notify lease: surface labels elect an owner, policy is settable, frames
//! republish when the roster changes.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use time::macros::datetime;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tower::ServiceExt;

use nerve_hub::HubRuntime;
use nerve_hub::clock::FakeClock;
use nerve_hub::state::{JobStore, PidProbe, PidState};

const MACHINE_ALIAS: &str = "test-mac";
const FRAME_WAIT: Duration = Duration::from_secs(2);

fn runtime() -> HubRuntime {
    let clock = Arc::new(FakeClock::new(datetime!(2026-07-19 00:00:00 UTC)));
    HubRuntime::new(JobStore::new(
        clock,
        MACHINE_ALIAS.to_string(),
        Arc::new(EveryPidAlive),
    ))
}

struct EveryPidAlive;
impl PidProbe for EveryPidAlive {
    fn state(&self, _pid: i32) -> PidState {
        PidState::Alive
    }
}

fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 54_321))
}

fn request(method: &str, uri: &str, body: Body) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(body)
        .expect("request");
    request.extensions_mut().insert(ConnectInfo(loopback()));
    request
}

struct Reply {
    status: StatusCode,
    body: Value,
}

async fn call(runtime: &HubRuntime, request: Request<Body>) -> Reply {
    let response = runtime
        .router()
        .oneshot(request)
        .await
        .expect("router answers");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    Reply {
        status,
        body: serde_json::from_slice(&bytes).expect("json"),
    }
}

async fn serve(runtime: &HubRuntime) -> SocketAddr {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind");
    let address = listener.local_addr().expect("addr");
    let service = runtime
        .router()
        .into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move {
        axum::serve(listener, service).await.expect("serve");
    });
    address
}

struct SseClient {
    reader: BufReader<TcpStream>,
}

impl SseClient {
    async fn connect(address: SocketAddr, query: &str) -> Self {
        let mut socket = TcpStream::connect(address).await.expect("connect");
        let request = format!(
            "GET /v1/stream{query} HTTP/1.1\r\nHost: {address}\r\nAccept: text/event-stream\r\n\r\n"
        );
        socket.write_all(request.as_bytes()).await.expect("write");
        let mut reader = BufReader::new(socket);
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("header");
            if line == "\r\n" || line == "\n" {
                break;
            }
        }
        Self { reader }
    }

    async fn frame(&mut self) -> Value {
        timeout(FRAME_WAIT, async {
            loop {
                let mut line = String::new();
                self.reader.read_line(&mut line).await.expect("line");
                let Some(payload) = line.trim_end().strip_prefix("data:") else {
                    continue;
                };
                return serde_json::from_str(payload.trim()).expect("frame json");
            }
        })
        .await
        .expect("frame arrived")
    }

    async fn until(&mut self, check: impl Fn(&Value) -> bool) -> Value {
        let deadline = tokio::time::Instant::now() + FRAME_WAIT;
        loop {
            let frame = self.frame().await;
            if check(&frame) {
                return frame;
            }
            if tokio::time::Instant::now() > deadline {
                panic!("lease never reached the expected state: {frame}");
            }
        }
    }
}

#[tokio::test]
async fn test_put_notify_all_clears_the_owner() {
    let runtime = runtime();
    let _hold = runtime.subscribe();
    let reply = call(
        &runtime,
        request("PUT", "/v1/notify", Body::from(r#"{"policy":"all"}"#)),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["policy"], "all");
    assert!(reply.body["owner"].is_null());
}

#[tokio::test]
async fn test_dropping_macos_hands_the_lease_to_vscode() {
    let runtime = runtime();
    let address = serve(&runtime).await;

    let macos = SseClient::connect(address, "?surface=macos").await;
    let mut vscode = SseClient::connect(address, "?surface=vscode").await;
    let _ = vscode.until(|frame| frame["notify"]["watchers"] == 2).await;
    drop(macos);
    let frame = vscode
        .until(|frame| frame["notify"]["owner"] == "vscode" && frame["notify"]["watchers"] == 1)
        .await;
    assert_eq!(frame["notify"]["owner"], "vscode");
}
