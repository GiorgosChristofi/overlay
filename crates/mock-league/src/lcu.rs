//! The fake League client (LCU): the REST endpoints the app uses and the
//! WAMP 1.0 WebSocket that pushes gameflow and champ-select events.
//!
//! It enforces what the real client enforces where the app depends on it:
//! Basic auth with the lockfile token, the rune page limit, which pages may
//! be deleted, and item sets only for the logged-in summoner. Anything else
//! answers 404 and is recorded as unknown, so tests can assert the app never
//! calls endpoints it has no business calling.

use crate::state::{Push, Recorded, RunePage, Shared};
use axum::body::{to_bytes, Body};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;

type Shr = State<Arc<Shared>>;

/// Request bodies larger than this are refused; the app sends a few KB.
const MAX_BODY: usize = 1024 * 1024;

pub(crate) fn router(shared: Arc<Shared>) -> Router {
    Router::new()
        .route("/", get(websocket))
        .route("/lol-summoner/v1/current-summoner", get(summoner))
        .route("/lol-gameflow/v1/gameflow-phase", get(phase))
        .route("/lol-champ-select/v1/session", get(session))
        .route(
            "/lol-champ-select/v1/session/my-selection",
            patch(my_selection),
        )
        .route("/lol-perks/v1/pages", get(pages).post(create_page))
        .route("/lol-perks/v1/pages/{id}", delete(delete_page))
        .route("/lol-perks/v1/inventory", get(inventory))
        .route(
            "/lol-item-sets/v1/item-sets/{summoner_id}/sets",
            get(item_sets).put(put_item_sets),
        )
        .fallback(unknown)
        .layer(middleware::from_fn_with_state(
            shared.clone(),
            auth_and_record,
        ))
        .with_state(shared)
}

fn error(status: StatusCode, message: &str) -> Response {
    let body =
        json!({ "errorCode": "RPC_ERROR", "httpStatus": status.as_u16(), "message": message });
    (status, Json(body)).into_response()
}

/// Checks the `riot:<token>` credentials and records every request.
async fn auth_and_record(State(shared): Shr, request: Request, next: Next) -> Response {
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .is_some_and(|v| v.as_bytes() == shared.auth.as_bytes());
    let method = request.method().to_string();
    let path = request.uri().path().to_owned();
    let (parts, body) = request.into_parts();
    let Ok(bytes) = to_bytes(body, MAX_BODY).await else {
        return error(StatusCode::PAYLOAD_TOO_LARGE, "body too large");
    };
    let body_json = serde_json::from_slice::<Value>(&bytes).ok();

    let response = if authorized {
        next.run(Request::from_parts(parts, Body::from(bytes)))
            .await
    } else {
        error(StatusCode::UNAUTHORIZED, "missing or wrong credentials")
    };
    let status = response.status().as_u16();
    let known = response.headers().get(UNKNOWN_HEADER).is_none();
    shared.state().record(Recorded {
        server: "lcu",
        method,
        path,
        status,
        authorized,
        known,
        body: body_json,
    });
    response
}

const UNKNOWN_HEADER: &str = "x-mock-unknown";

async fn unknown() -> Response {
    let mut response = error(
        StatusCode::NOT_FOUND,
        "the mock does not implement this endpoint",
    );
    response
        .headers_mut()
        .insert(UNKNOWN_HEADER, header::HeaderValue::from_static("1"));
    response
}

async fn summoner(State(shared): Shr) -> Json<Value> {
    Json(shared.state().summoner.clone())
}

async fn phase(State(shared): Shr) -> Json<Value> {
    Json(json!(shared.state().phase))
}

async fn session(State(shared): Shr) -> Response {
    match shared.state().champ_select.clone() {
        Some(session) => Json(session).into_response(),
        None => error(StatusCode::NOT_FOUND, "No active delegate"),
    }
}

/// Sets summoner spells; like the client, pushes the updated session.
async fn my_selection(State(shared): Shr, Json(body): Json<Value>) -> Response {
    let spell = |key: &str| body.get(key).and_then(Value::as_i64);
    let (Some(s1), Some(s2)) = (spell("spell1Id"), spell("spell2Id")) else {
        return error(
            StatusCode::BAD_REQUEST,
            "spell1Id and spell2Id are required",
        );
    };
    let session = {
        let mut state = shared.state();
        let Some(mut session) = state.champ_select.clone() else {
            return error(StatusCode::NOT_FOUND, "No active delegate");
        };
        state.my_selection = Some([s1, s2]);
        let cell = session.get("localPlayerCellId").cloned();
        if let Some(team) = session.get_mut("myTeam").and_then(Value::as_array_mut) {
            for member in team.iter_mut().filter(|m| m.get("cellId") == cell.as_ref()) {
                member["spell1Id"] = json!(s1);
                member["spell2Id"] = json!(s2);
            }
        }
        session
    };
    shared.set_champ_select(Some(session));
    StatusCode::NO_CONTENT.into_response()
}

async fn pages(State(shared): Shr) -> Json<Vec<RunePage>> {
    Json(shared.state().rune_pages.clone())
}

async fn inventory(State(shared): Shr) -> Json<Value> {
    Json(
        json!({ "ownedPageCount": shared.state().owned_page_count, "isCustomPageCreationUnlocked": true }),
    )
}

async fn create_page(State(shared): Shr, Json(body): Json<Value>) -> Response {
    let mut state = shared.state();
    let editable = state.rune_pages.iter().filter(|p| p.is_deletable).count();
    if editable >= state.owned_page_count as usize {
        return error(StatusCode::BAD_REQUEST, "Max pages reached");
    }
    let perks: Vec<i64> = body
        .get("selectedPerkIds")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default();
    if perks.len() != 9 {
        return error(StatusCode::BAD_REQUEST, "a page needs 9 perks");
    }
    let id = state.next_page_id;
    state.next_page_id += 1;
    for page in &mut state.rune_pages {
        page.current = false;
    }
    let page = RunePage {
        id,
        name: body
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        is_editable: true,
        is_deletable: true,
        current: true,
        primary_style_id: body
            .get("primaryStyleId")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        sub_style_id: body
            .get("subStyleId")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        selected_perk_ids: perks,
    };
    state.rune_pages.push(page.clone());
    Json(page).into_response()
}

async fn delete_page(State(shared): Shr, Path(id): Path<i64>) -> Response {
    let mut state = shared.state();
    match state.rune_pages.iter().position(|p| p.id == id) {
        None => error(StatusCode::NOT_FOUND, "page not found"),
        Some(i) if !state.rune_pages[i].is_deletable => {
            error(StatusCode::BAD_REQUEST, "page cannot be deleted")
        }
        Some(i) => {
            state.rune_pages.remove(i);
            StatusCode::NO_CONTENT.into_response()
        }
    }
}

async fn item_sets(State(shared): Shr, Path(summoner_id): Path<i64>) -> Response {
    let state = shared.state();
    if summoner_id != state.summoner_id() {
        return error(StatusCode::BAD_REQUEST, "not the current summoner");
    }
    let doc = state
        .item_sets
        .get(&summoner_id)
        .cloned()
        .unwrap_or_else(|| json!({ "accountId": summoner_id, "itemSets": [], "timestamp": 0 }));
    Json(doc).into_response()
}

async fn put_item_sets(
    State(shared): Shr,
    Path(summoner_id): Path<i64>,
    Json(doc): Json<Value>,
) -> Response {
    let mut state = shared.state();
    if summoner_id != state.summoner_id() {
        return error(StatusCode::BAD_REQUEST, "not the current summoner");
    }
    if !doc.get("itemSets").is_some_and(Value::is_array) {
        return error(StatusCode::BAD_REQUEST, "itemSets must be an array");
    }
    state.item_sets.insert(summoner_id, doc);
    StatusCode::CREATED.into_response()
}

async fn websocket(State(shared): Shr, ws: WebSocketUpgrade) -> Response {
    ws.protocols(["wamp"])
        .on_upgrade(move |socket| wamp(socket, shared))
}

/// One WAMP session: answers subscribes and forwards events for subscribed topics.
async fn wamp(mut socket: WebSocket, shared: Arc<Shared>) {
    let mut pushes = shared.push.subscribe();
    let welcome = json!([0, "mock-session", 1, "mock-league"]).to_string();
    if socket.send(Message::Text(welcome.into())).await.is_err() {
        return;
    }
    let mut topics: HashSet<String> = HashSet::new();
    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(Value::Array(parts)) = serde_json::from_str::<Value>(text.as_str()) {
                        if parts.first().and_then(Value::as_u64) == Some(5) {
                            if let Some(topic) = parts.get(1).and_then(Value::as_str) {
                                topics.insert(topic.to_owned());
                            }
                        }
                    }
                }
                Some(Ok(Message::Close(_)) | Err(_)) | None => return,
                Some(Ok(_)) => {}
            },
            push = pushes.recv() => match push {
                Ok(Push::Event { topic, uri, event_type, data }) if topics.contains(topic) => {
                    let frame = json!([8, topic, { "data": data, "eventType": event_type, "uri": uri }]);
                    if socket.send(Message::Text(frame.to_string().into())).await.is_err() {
                        return;
                    }
                }
                Ok(Push::CloseAll) | Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    let _ = socket.send(Message::Close(None)).await;
                    return;
                }
                Ok(Push::Event { .. }) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            },
        }
    }
}
