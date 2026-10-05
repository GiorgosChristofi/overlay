//! Keeps a connection to the League client alive and turns its WebSocket
//! events into [`LcuEvent`]s. Survives client restarts: on any failure it
//! reports `Disconnected`, waits, and starts discovery again.

use crate::http::{basic_auth, LcuHttp};
use crate::lockfile::LockfileInfo;
use crate::tls::riot_client_config;
use crate::wamp::{self, JsonApiEvent};
use domain::champ_select::ChampSelectSession;
use domain::phase::GameflowPhase;
use domain::ports::{LcuEvent, LcuPort};
use futures_util::{SinkExt, StreamExt};
use rustls::ClientConfig;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{
    HeaderValue, AUTHORIZATION, SEC_WEBSOCKET_PROTOCOL,
};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::Connector;
use tokio_util::sync::CancellationToken;

type Discover = Arc<dyn Fn() -> Option<LockfileInfo> + Send + Sync>;

#[derive(Clone)]
pub struct ConnectorConfig {
    /// PEM of Riot's root certificate. Required for the real (https) client.
    pub riot_root_pem: Option<Vec<u8>>,
    /// How long to wait between discovery attempts and after a disconnect.
    pub retry_interval: Duration,
    /// How to find the client. Real app: [`crate::discover`]; tests inject a fake.
    pub discover: Discover,
}

impl fmt::Debug for ConnectorConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectorConfig")
            .field("has_riot_root", &self.riot_root_pem.is_some())
            .field("retry_interval", &self.retry_interval)
            .finish_non_exhaustive()
    }
}

impl ConnectorConfig {
    pub fn new(riot_root_pem: Option<Vec<u8>>) -> Self {
        Self {
            riot_root_pem,
            retry_interval: Duration::from_secs(2),
            discover: Arc::new(crate::discovery::discover),
        }
    }
}

/// Runs until `cancel` fires. Sends events on `tx`.
pub async fn run_connector(
    config: ConnectorConfig,
    tx: mpsc::Sender<LcuEvent>,
    cancel: CancellationToken,
) {
    let tls = match config
        .riot_root_pem
        .as_deref()
        .map(riot_client_config)
        .transpose()
    {
        Ok(tls) => tls,
        Err(e) => {
            tracing::error!(error = %e, "cannot build TLS config; only plain http clients will work");
            None
        }
    };

    loop {
        if cancel.is_cancelled() {
            return;
        }
        if let Some(info) = (config.discover)() {
            match session(&info, tls.clone(), &tx, &cancel).await {
                Ok(()) => tracing::info!("LCU session ended"),
                Err(e) => tracing::debug!(error = %e, "LCU session not available"),
            }
        }
        tokio::select! {
            () = cancel.cancelled() => return,
            () = tokio::time::sleep(config.retry_interval) => {}
        }
    }
}

/// One connected session. Returns when the socket closes or fails.
async fn session(
    info: &LockfileInfo,
    tls: Option<Arc<ClientConfig>>,
    tx: &mpsc::Sender<LcuEvent>,
    cancel: &CancellationToken,
) -> Result<(), String> {
    let http = LcuHttp::new(info, tls.clone()).map_err(|e| e.to_string())?;
    // The client opens its port before it is ready; wait until it answers.
    let phase = http.gameflow_phase().await.map_err(|e| e.to_string())?;

    let mut request = info
        .ws_url()
        .into_client_request()
        .map_err(|e| e.to_string())?;
    let headers = request.headers_mut();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&basic_auth(&info.password)).map_err(|e| e.to_string())?,
    );
    headers.insert(SEC_WEBSOCKET_PROTOCOL, HeaderValue::from_static("wamp"));
    let connector = if info.is_tls() {
        tls.map(Connector::Rustls)
    } else {
        None
    };
    let (mut ws, _) =
        tokio_tungstenite::connect_async_tls_with_config(request, None, false, connector)
            .await
            .map_err(|e| e.to_string())?;

    for topic in [wamp::TOPIC_GAMEFLOW_PHASE, wamp::TOPIC_CHAMP_SELECT] {
        ws.send(Message::text(wamp::subscribe_message(topic)))
            .await
            .map_err(|e| e.to_string())?;
    }

    tracing::info!(port = info.port, "connected to the League client");
    let port: Arc<dyn LcuPort> = Arc::new(http.clone());
    let in_champ_select = phase == GameflowPhase::ChampSelect;
    send(tx, LcuEvent::Connected(port)).await?;
    send(tx, LcuEvent::Phase(phase)).await?;
    if in_champ_select {
        // Started the app mid champ select: catch up on the current session.
        if let Ok(session) = http.champ_select_session().await {
            send(tx, LcuEvent::ChampSelect(session)).await?;
        }
    }

    let result = loop {
        let frame = tokio::select! {
            () = cancel.cancelled() => break Ok(()),
            frame = ws.next() => frame,
        };
        match frame {
            Some(Ok(Message::Text(text))) => {
                if let Some(event) = wamp::parse_event(text.as_str()).and_then(to_lcu_event) {
                    send(tx, event).await?;
                }
            }
            Some(Ok(Message::Close(_))) | None => break Ok(()),
            Some(Ok(_)) => {}
            Some(Err(e)) => break Err(e.to_string()),
        }
    };
    send(tx, LcuEvent::Disconnected).await?;
    result
}

async fn send(tx: &mpsc::Sender<LcuEvent>, event: LcuEvent) -> Result<(), String> {
    tx.send(event)
        .await
        .map_err(|_| "event receiver dropped".to_string())
}

/// Maps a WAMP payload to a domain event. Unknown or malformed payloads are dropped.
pub fn to_lcu_event(event: JsonApiEvent) -> Option<LcuEvent> {
    match event.uri.as_str() {
        wamp::URI_GAMEFLOW_PHASE => serde_json::from_value::<GameflowPhase>(event.data)
            .ok()
            .map(LcuEvent::Phase),
        wamp::URI_CHAMP_SELECT if event.event_type == "Delete" => Some(LcuEvent::ChampSelect(None)),
        wamp::URI_CHAMP_SELECT => match serde_json::from_value::<ChampSelectSession>(event.data) {
            Ok(session) => Some(LcuEvent::ChampSelect(Some(session))),
            Err(e) => {
                tracing::warn!(error = %e, "could not parse champ select session");
                None
            }
        },
        _ => None,
    }
}
