use crate::protocol::{Envelope, MessageType};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncomingDecision {
    AuthAccepted,
    AuthFailed(String),
    StoreNotification(Value),
    StoreBatch(Value),
    RemoveNotification(String),
    RemoveBatch(Vec<String>),
    PingReceived,
    StatusUpdate(Value),
    AppInventory(Value),
    RulesAck(Value),
    ManualDisconnect,
    Unknown,
}

pub fn handle_envelope(env: &Envelope, expected_pairing_key: &str) -> IncomingDecision {
    match env.r#type {
        MessageType::Auth => {
            let Some(key) = env.payload.get("pairingKey").and_then(|v| v.as_str()) else {
                return IncomingDecision::AuthFailed("missing pairingKey".into());
            };
            if key == expected_pairing_key {
                IncomingDecision::AuthAccepted
            } else {
                IncomingDecision::AuthFailed("invalid pairing key".into())
            }
        }
        MessageType::Notification => IncomingDecision::StoreNotification(env.payload.clone()),
        MessageType::NotificationBatch => IncomingDecision::StoreBatch(env.payload.clone()),
        MessageType::Dismissal => env
            .payload
            .get("id")
            .and_then(|v| v.as_str())
            .map(|id| IncomingDecision::RemoveNotification(id.to_string()))
            .unwrap_or(IncomingDecision::Unknown),
        MessageType::DismissalBatch => {
            let ids: Vec<String> = env
                .payload
                .get("ids")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|i| i.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            IncomingDecision::RemoveBatch(ids)
        }
        MessageType::Ping => IncomingDecision::PingReceived,
        MessageType::Status => IncomingDecision::StatusUpdate(env.payload.clone()),
        MessageType::AppInventory => IncomingDecision::AppInventory(env.payload.clone()),
        MessageType::RulesAck => IncomingDecision::RulesAck(env.payload.clone()),
        MessageType::Unpair => IncomingDecision::ManualDisconnect,
        MessageType::AuthOk
        | MessageType::AuthFailed
        | MessageType::NotificationAck
        | MessageType::Pong
        | MessageType::RulesUpdate
        | MessageType::DesktopAction
        | MessageType::Encrypted => IncomingDecision::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn env(t: MessageType, payload: Value) -> Envelope {
        Envelope {
            version: 1,
            r#type: t,
            payload,
        }
    }

    #[test]
    fn accepts_matching_key() {
        let e = env(MessageType::Auth, json!({ "pairingKey": "k" }));
        assert_eq!(handle_envelope(&e, "k"), IncomingDecision::AuthAccepted);
    }

    #[test]
    fn rejects_mismatch() {
        let e = env(MessageType::Auth, json!({ "pairingKey": "x" }));
        assert!(matches!(
            handle_envelope(&e, "k"),
            IncomingDecision::AuthFailed(_)
        ));
    }

    #[test]
    fn parses_dismissal_id() {
        let e = env(MessageType::Dismissal, json!({ "id": "abc" }));
        assert_eq!(
            handle_envelope(&e, "k"),
            IncomingDecision::RemoveNotification("abc".into())
        );
    }

    #[test]
    fn parses_dismissal_batch() {
        let e = env(MessageType::DismissalBatch, json!({ "ids": ["a", "b"] }));
        assert_eq!(
            handle_envelope(&e, "k"),
            IncomingDecision::RemoveBatch(vec!["a".into(), "b".into()])
        );
    }

    #[test]
    fn routes_app_inventory_payload() {
        let payload = json!({
            "apps": [
                {
                    "packageName": "com.whatsapp",
                    "label": "WhatsApp",
                    "category": "messaging",
                    "notificationsSeen": 3,
                    "lastSeenAt": 123
                }
            ]
        });
        let e = env(MessageType::AppInventory, payload.clone());
        assert_eq!(
            handle_envelope(&e, "k"),
            IncomingDecision::AppInventory(payload)
        );
    }

    #[test]
    fn routes_unpair_as_manual_disconnect() {
        let e = env(MessageType::Unpair, json!({ "reason": "manual_disconnect" }));
        assert_eq!(handle_envelope(&e, "k"), IncomingDecision::ManualDisconnect);
    }
}
