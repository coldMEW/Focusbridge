use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRow {
    pub id: String,
    pub app_name: String,
    pub package_name: String,
    pub sender: String,
    pub message: String,
    pub timestamp: i64,
    pub received_at: i64,
    pub status: String,
    pub priority: i32,
    pub content_hidden: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedDeviceRow {
    pub id: i64,
    pub device_name: String,
    pub device_id: String,
    pub pairing_key: String,
    pub mode: String,
    pub endpoint: Option<String>,
    pub cert_fingerprint: Option<String>,
    pub is_active: i32,
    pub created_at: i64,
    pub last_connected_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppRuleRow {
    pub package_name: String,
    pub label: String,
    pub category: String,
    pub icon_data_url: String,
    pub notifications_seen: i64,
    pub last_seen_at: i64,
    pub muted: i32,
    pub priority: i32,
    pub study_safe: i32,
    pub updated_at: i64,
}
