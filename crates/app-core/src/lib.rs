//! Application services. Everything here talks to the outside world only
//! through `domain::ports`, so it runs unchanged against fakes in tests.

mod core;
mod import;
mod settings_store;

pub use crate::core::{Core, CoreError, EventSink};
pub use import::{run_import, ImportReport, StepOutcome};
pub use settings_store::{SettingsError, SettingsStore};
