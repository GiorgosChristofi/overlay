//! Pure domain logic for the overlay.
//!
//! This crate has no I/O. It defines the data the app reasons about, the rules
//! that operate on it, and the port traits that adapter crates implement.

pub mod builds;
pub mod champ_select;
pub mod error;
pub mod item_set;
pub mod objectives;
pub mod phase;
pub mod ports;
pub mod runes;
pub mod session;
pub mod settings;
pub mod shopping;
pub mod spells;
pub mod static_catalog;
pub mod summoner;

pub use error::PortError;
