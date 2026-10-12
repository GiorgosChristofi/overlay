//! The fake game's Live Client Data API (`https://127.0.0.1:2999`). Like the
//! real one it needs no credentials and answers 404 while no game data exists
//! (loading screen); the app treats that the same as "no game".

use crate::state::{Recorded, Shared};
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use serde_json::json;
use std::sync::Arc;

pub(crate) fn router(shared: Arc<Shared>) -> Router {
    Router::new().fallback(handle).with_state(shared)
}

async fn handle(State(shared): State<Arc<Shared>>, request: Request) -> Response {
    let path = request.uri().path().to_owned();
    let mut state = shared.state();
    let (status, known, response) = match (path.as_str(), &state.live) {
        ("/liveclientdata/allgamedata", Some(data)) => {
            (StatusCode::OK, true, Json(data.clone()).into_response())
        }
        ("/liveclientdata/allgamedata", None) => (StatusCode::NOT_FOUND, true, not_found()),
        _ => (StatusCode::NOT_FOUND, false, not_found()),
    };
    state.record(Recorded {
        server: "live",
        method: request.method().to_string(),
        path,
        status: status.as_u16(),
        authorized: true,
        known,
        body: None,
    });
    response
}

fn not_found() -> Response {
    let body =
        json!({ "errorCode": "RESOURCE_NOT_FOUND", "httpStatus": 404, "message": "No game data" });
    (StatusCode::NOT_FOUND, Json(body)).into_response()
}
