//! The LCU's WebSocket speaks WAMP 1.0. We only subscribe and receive events.

use serde::Deserialize;
use serde_json::Value;

pub const TOPIC_GAMEFLOW_PHASE: &str = "OnJsonApiEvent_lol-gameflow_v1_gameflow-phase";
pub const TOPIC_CHAMP_SELECT: &str = "OnJsonApiEvent_lol-champ-select_v1_session";

pub const URI_GAMEFLOW_PHASE: &str = "/lol-gameflow/v1/gameflow-phase";
pub const URI_CHAMP_SELECT: &str = "/lol-champ-select/v1/session";

const OPCODE_SUBSCRIBE: u8 = 5;
const OPCODE_EVENT: u64 = 8;

pub fn subscribe_message(topic: &str) -> String {
    serde_json::json!([OPCODE_SUBSCRIBE, topic]).to_string()
}

/// The payload of an `OnJsonApiEvent` message.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonApiEvent {
    pub uri: String,
    /// `Create`, `Update` or `Delete`.
    pub event_type: String,
    #[serde(default)]
    pub data: Value,
}

/// Parses an incoming text frame; returns `None` for anything that is not an event.
pub fn parse_event(text: &str) -> Option<JsonApiEvent> {
    let Value::Array(parts) = serde_json::from_str::<Value>(text).ok()? else {
        return None;
    };
    if parts.first()?.as_u64()? != OPCODE_EVENT {
        return None;
    }
    serde_json::from_value(parts.get(2)?.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_subscribe_message() {
        assert_eq!(
            subscribe_message(TOPIC_GAMEFLOW_PHASE),
            r#"[5,"OnJsonApiEvent_lol-gameflow_v1_gameflow-phase"]"#
        );
    }

    #[test]
    fn parses_event_frame() {
        let frame = r#"[8,"OnJsonApiEvent_lol-gameflow_v1_gameflow-phase",{"data":"ChampSelect","eventType":"Update","uri":"/lol-gameflow/v1/gameflow-phase"}]"#;
        let e = parse_event(frame).unwrap();
        assert_eq!(e.uri, URI_GAMEFLOW_PHASE);
        assert_eq!(e.event_type, "Update");
        assert_eq!(e.data, Value::String("ChampSelect".into()));
    }

    #[test]
    fn ignores_welcome_and_garbage() {
        assert!(parse_event(r#"[0,"session",1,"server"]"#).is_none());
        assert!(parse_event("not json").is_none());
        assert!(parse_event("{}").is_none());
    }
}
