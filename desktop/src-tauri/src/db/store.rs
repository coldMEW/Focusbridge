use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use std::path::Path;

use super::models::NotificationRow;

pub fn init(db_path: &Path) -> Result<()> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).context("create app data directory")?;
    }
    let conn = Connection::open(db_path).context("open desktop sqlite database")?;
    conn.execute_batch(include_str!("../../migrations/001_initial.sql"))
        .context("apply desktop sqlite schema")?;
    Ok(())
}

pub fn upsert_notification(db_path: &Path, payload: &Value) -> Result<NotificationRow> {
    let row = notification_from_payload(payload);
    let conn = Connection::open(db_path).context("open desktop sqlite database")?;
    conn.execute(
        "INSERT INTO notifications (
            id, app_name, package_name, sender, message, timestamp, received_at, status, priority, content_hidden
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'NEW', ?8, ?9)
        ON CONFLICT(id) DO UPDATE SET
            app_name=excluded.app_name,
            package_name=excluded.package_name,
            sender=excluded.sender,
            message=excluded.message,
            timestamp=excluded.timestamp,
            priority=excluded.priority,
            content_hidden=excluded.content_hidden",
        params![
            row.id,
            row.app_name,
            row.package_name,
            row.sender,
            row.message,
            row.timestamp,
            row.received_at,
            row.priority,
            row.content_hidden,
        ],
    )
    .context("upsert notification")?;
    Ok(row)
}

pub fn mark_status(db_path: &Path, id: &str, status: &str) -> Result<()> {
    let conn = Connection::open(db_path).context("open desktop sqlite database")?;
    conn.execute(
        "UPDATE notifications SET status = ?1 WHERE id = ?2",
        params![status, id],
    )
    .context("update notification status")?;
    Ok(())
}

pub fn dismiss_notification(db_path: &Path, id: &str) -> Result<()> {
    mark_status(db_path, id, "READ")
}

pub fn set_setting(db_path: &Path, key: &str, value: &str) -> Result<()> {
    let conn = Connection::open(db_path).context("open desktop sqlite database")?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    )
    .context("set setting")?;
    Ok(())
}

pub fn get_setting(db_path: &Path, key: &str) -> Result<Option<String>> {
    let conn = Connection::open(db_path).context("open desktop sqlite database")?;
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .context("get setting")
}

pub fn save_pairing(
    db_path: &Path,
    device_name: &str,
    device_id: &str,
    pairing_key: &str,
    endpoint: &str,
    cert_fingerprint: &str,
) -> Result<()> {
    let conn = Connection::open(db_path).context("open desktop sqlite database")?;
    conn.execute(
        "INSERT INTO paired_devices (
            device_name, device_id, pairing_key, mode, endpoint, cert_fingerprint, is_active, created_at
        ) VALUES (?1, ?2, ?3, 'LOCAL', ?4, ?5, 1, ?6)
        ON CONFLICT(device_id) DO UPDATE SET
            pairing_key=excluded.pairing_key,
            endpoint=excluded.endpoint,
            cert_fingerprint=excluded.cert_fingerprint,
            is_active=1",
        params![
            device_name,
            device_id,
            pairing_key,
            endpoint,
            cert_fingerprint,
            now_millis(),
        ],
    )
    .context("save pairing")?;
    Ok(())
}

fn notification_from_payload(payload: &Value) -> NotificationRow {
    let now = now_millis();
    NotificationRow {
        id: string_field(payload, "id", &format!("notification-{now}")),
        app_name: string_field(payload, "appName", "Unknown"),
        package_name: string_field(payload, "packageName", "unknown"),
        sender: nullable_string_field(payload, "sender"),
        message: nullable_string_field(payload, "message"),
        timestamp: payload
            .get("timestamp")
            .and_then(Value::as_i64)
            .unwrap_or(now),
        received_at: now,
        status: "NEW".to_string(),
        priority: priority_score(payload.get("priority")),
        content_hidden: payload
            .get("contentHidden")
            .and_then(Value::as_bool)
            .unwrap_or(false) as i32,
    }
}

fn string_field(payload: &Value, key: &str, default: &str) -> String {
    payload
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn nullable_string_field(payload: &Value, key: &str) -> String {
    payload
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn priority_score(value: Option<&Value>) -> i32 {
    match value {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(30).clamp(0, 100) as i32,
        Some(Value::String(s)) if s == "URGENT" => 100,
        Some(Value::String(s)) if s == "HIGH" => 80,
        Some(Value::String(s)) if s == "LOW" => 10,
        Some(Value::String(_)) => 30,
        _ => 30,
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
