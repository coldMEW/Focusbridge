use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MessageType {
    Auth,
    AuthOk,
    AuthFailed,
    Notification,
    NotificationBatch,
    Dismissal,
    DismissalBatch,
    Ping,
    Pong,
    Status,
    DesktopAction,
    Unpair,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub version: u32,
    #[serde(rename = "type")]
    pub r#type: MessageType,
    pub payload: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn roundtrip() {
        let env = Envelope {
            version: 1,
            r#type: MessageType::Notification,
            payload: json!({}),
        };
        let s = serde_json::to_string(&env).unwrap();
        let back: Envelope = serde_json::from_str(&s).unwrap();
        assert_eq!(back.r#type, MessageType::Notification);
    }

    #[test]
    fn decodes_snake_case() {
        let raw = r#"{"version":1,"type":"AUTH_OK","payload":{}}"#;
        let e: Envelope = serde_json::from_str(raw).unwrap();
        assert_eq!(e.r#type, MessageType::AuthOk);
    }
}
