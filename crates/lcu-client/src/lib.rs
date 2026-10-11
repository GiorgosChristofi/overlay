//! Adapters for the League Client API (LCU) and, while a game runs, the
//! game's Live Client Data API ([`live`]).
//!
//! The LCU is unsupported by Riot and changes without notice, so this crate
//! parses tolerantly and keeps every endpoint path in one place ([`http`]).

pub mod connection;
pub mod discovery;
pub mod http;
pub mod live;
pub mod lockfile;
pub mod tls;
pub mod wamp;

pub use connection::{run_connector, ConnectorConfig};
pub use discovery::discover;
pub use http::LcuHttp;
pub use live::{LiveClient, LIVE_CLIENT_URL};
pub use lockfile::LockfileInfo;
