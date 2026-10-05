//! A fake League client: REST and the WAMP WebSocket on one port, like the
//! real LCU. Responses are canned per (method, path); every request is recorded.

#![allow(dead_code, clippy::unwrap_used, clippy::panic)]

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use lcu_client::LockfileInfo;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;

pub const PASSWORD: &str = "test-token";
const CLOSE: &str = "__close__";

#[derive(Debug, Clone, PartialEq)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub body: String,
}

/// (method, path) -> (status, body)
type Canned = HashMap<(String, String), (u16, String)>;

#[derive(Clone)]
struct Shared {
    responses: Arc<Mutex<Canned>>,
    requests: Arc<Mutex<Vec<Recorded>>>,
    subscriptions: Arc<Mutex<Vec<String>>>,
    frames: broadcast::Sender<String>,
}

pub struct FakeLcu {
    pub port: u16,
    shared: Shared,
}

impl FakeLcu {
    pub async fn start() -> Self {
        let (frames, _) = broadcast::channel(64);
        let shared = Shared {
            responses: Arc::default(),
            requests: Arc::default(),
            subscriptions: Arc::default(),
            frames,
        };
        let app = Router::new()
            .route("/", get(ws_handler))
            .fallback(rest_handler)
            .with_state(shared.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { port, shared }
    }

    pub fn lockfile(&self) -> LockfileInfo {
        LockfileInfo {
            pid: 1,
            port: self.port,
            password: PASSWORD.into(),
            protocol: "http".into(),
        }
    }

    pub fn respond(&self, method: &str, path: &str, status: u16, body: &str) {
        self.shared
            .responses
            .lock()
            .unwrap()
            .insert((method.into(), path.into()), (status, body.into()));
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.shared.requests.lock().unwrap().clone()
    }

    pub fn subscriptions(&self) -> Vec<String> {
        self.shared.subscriptions.lock().unwrap().clone()
    }

    /// Sends a WAMP event frame to every connected socket.
    pub fn push_event(&self, topic: &str, uri: &str, event_type: &str, data: serde_json::Value) {
        let frame =
            serde_json::json!([8, topic, { "uri": uri, "eventType": event_type, "data": data }]);
        self.shared.frames.send(frame.to_string()).ok();
    }

    /// Closes every connected socket, like a client restart.
    pub fn close_sockets(&self) {
        self.shared.frames.send(CLOSE.into()).ok();
    }

    pub async fn wait_for_subscriptions(&self, n: usize) {
        for _ in 0..200 {
            if self.subscriptions().len() >= n {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("client never subscribed");
    }
}

fn authorized(headers: &HeaderMap) -> bool {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == lcu_client::http::basic_auth(PASSWORD))
}

async fn rest_handler(
    State(s): State<Shared>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !authorized(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let path = uri.path().to_owned();
    s.requests.lock().unwrap().push(Recorded {
        method: method.to_string(),
        path: path.clone(),
        body: String::from_utf8_lossy(&body).into_owned(),
    });
    let canned = s
        .responses
        .lock()
        .unwrap()
        .get(&(method.to_string(), path))
        .cloned();
    match canned {
        Some((status, body)) => (
            StatusCode::from_u16(status).unwrap(),
            [("content-type", "application/json")],
            body,
        )
            .into_response(),
        None if method == Method::GET => StatusCode::NOT_FOUND.into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

async fn ws_handler(ws: WebSocketUpgrade, headers: HeaderMap, State(s): State<Shared>) -> Response {
    if !authorized(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    ws.protocols(["wamp"])
        .on_upgrade(move |socket| serve_socket(socket, s))
}

async fn serve_socket(socket: WebSocket, s: Shared) {
    let (mut sink, mut stream) = socket.split();
    let mut frames = s.frames.subscribe();
    let subs = s.subscriptions.clone();
    tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = stream.next().await {
            subs.lock().unwrap().push(text.to_string());
        }
    });
    while let Ok(frame) = frames.recv().await {
        if frame == CLOSE {
            sink.send(Message::Close(None)).await.ok();
            return;
        }
        if sink.send(Message::Text(frame.into())).await.is_err() {
            return;
        }
    }
}
