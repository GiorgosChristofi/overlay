//! Plain-HTTP control API for driving a running mock by hand or from a
//! script, e.g. `curl -d '{"phase":"ChampSelect"}' http://127.0.0.1:<port>/step`.
//! Listens on `127.0.0.1` only.
//!
//! - `POST /step` applies one [`Step`] (`wait` is ignored).
//! - `POST /steps` runs a list of steps, honouring `wait`, then answers.
//! - `GET /state` returns the [`crate::Snapshot`] as JSON.

use crate::{Driver, Step};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use std::sync::Arc;

pub(crate) fn router(driver: Arc<Driver>) -> Router {
    Router::new()
        .route("/step", post(step))
        .route("/steps", post(steps))
        .route("/state", get(state))
        .with_state(driver)
}

async fn step(State(driver): State<Arc<Driver>>, Json(step): Json<Step>) -> impl IntoResponse {
    driver.apply(&step);
    StatusCode::NO_CONTENT
}

async fn steps(
    State(driver): State<Arc<Driver>>,
    Json(steps): Json<Vec<Step>>,
) -> impl IntoResponse {
    driver.run(&steps).await;
    StatusCode::NO_CONTENT
}

async fn state(State(driver): State<Arc<Driver>>) -> impl IntoResponse {
    Json(driver.snapshot())
}
