//! Adapter for the League Client API (LCU).
//!
//! The LCU is unsupported by Riot and changes without notice, so this crate
//! parses tolerantly and keeps every endpoint path in one place ([`http`]).

pub mod connection;
pub mod discovery;
pub mod http;
pub mod lockfile;
pub mod tls;
pub mod wamp;

pub use connection::{run_connector, ConnectorConfig};
pub use discovery::discover;
pub use http::LcuHttp;
pub use lockfile::LockfileInfo;
