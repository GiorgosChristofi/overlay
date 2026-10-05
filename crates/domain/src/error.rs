use thiserror::Error;

/// Errors an adapter can report through a port.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PortError {
    #[error("the League client is not connected")]
    NotConnected,
    #[error("request failed with HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("could not decode response: {0}")]
    Decode(String),
    #[error("transport error: {0}")]
    Transport(String),
}
