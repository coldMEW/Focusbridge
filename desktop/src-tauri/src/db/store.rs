use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use std::path::Path;

use super::models::{AppRuleRow, NotificationRow};

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
    upsert_app_seen(
        &conn,
        &row.package_name,
        &row.app_name,
        &categorize_app(&row.package_name, &row.app_name),
        row.timestamp.max(row.received_at),
    )?;
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

pub fn list_app_rules(db_path: &Path) -> Result<Vec<AppRuleRow>> {
    let conn = Connection::open(db_path).context("open desktop sqlite database")?;
    let mut stmt = conn
        .prepare(
            "SELECT package_name, label, category, notifications_seen, last_seen_at, muted, priority, study_safe, updated_at
             FROM app_rules
             ORDER BY muted ASC, priority DESC, notifications_seen DESC, label COLLATE NOCASE ASC",
        )
        .context("prepare app rules list")?;

    let rows = stmt
        .query_map([], app_rule_from_row)
        .context("query app rules")?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("collect app rules")
}

pub fn set_app_rule_flag(
    db_path: &Path,
    package_name: &str,
    flag: &str,
    enabled: bool,
) -> Result<AppRuleRow> {
    let column = match flag {
        "muted" => "muted",
        "priority" => "priority",
        "study_safe" => "study_safe",
        other => anyhow::bail!("unsupported app rule flag: {other}"),
    };
    let conn = Connection::open(db_path).context("open desktop sqlite database")?;
    conn.execute(
        &format!(
            "INSERT INTO app_rules (package_name, label, category, updated_at)
             VALUES (?1, ?1, 'other', ?2)
             ON CONFLICT(package_name) DO NOTHING"
        ),
        params![package_name, now_millis()],
    )
    .context("ensure app rule row")?;
    conn.execute(
        &format!("UPDATE app_rules SET {column} = ?1, updated_at = ?2 WHERE package_name = ?3"),
        params![enabled as i32, now_millis(), package_name],
    )
    .context("update app rule flag")?;
    get_app_rule(&conn, package_name)
}

pub fn save_app_inventory(db_path: &Path, payload: &Value) -> Result<Vec<AppRuleRow>> {
    let conn = Connection::open(db_path).context("open desktop sqlite database")?;
    if let Some(apps) = payload.get("apps").and_then(Value::as_array) {
        for app in apps {
            let package_name = string_field(app, "packageName", "");
            if package_name.is_empty() {
                continue;
            }
            let label = string_field(app, "label", &package_name);
            let category = string_field(app, "category", &categorize_app(&package_name, &label));
            let notifications_seen = app
                .get("notificationsSeen")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            let last_seen_at = app
                .get("lastSeenAt")
                .and_then(Value::as_i64)
                .unwrap_or_else(now_millis);
            conn.execute(
                "INSERT INTO app_rules (
                    package_name, label, category, notifications_seen, last_seen_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(package_name) DO UPDATE SET
                    label=excluded.label,
                    category=excluded.category,
                    notifications_seen=MAX(app_rules.notifications_seen, excluded.notifications_seen),
                    last_seen_at=MAX(app_rules.last_seen_at, excluded.last_seen_at)",
                params![
                    package_name,
                    label,
                    category,
                    notifications_seen,
                    last_seen_at,
                    now_millis(),
                ],
            )
            .context("save app inventory row")?;
        }
    }
    list_app_rules(db_path)
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

pub fn list_notifications(db_path: &Path, limit: i64) -> Result<Vec<NotificationRow>> {
    let conn = Connection::open(db_path).context("open desktop sqlite database")?;
    let mut stmt = conn
        .prepare(
            "SELECT
                id, app_name, package_name, sender, message, timestamp, received_at, status, priority, content_hidden
             FROM notifications
             WHERE status != 'READ'
             ORDER BY timestamp DESC, received_at DESC
             LIMIT ?1",
        )
        .context("prepare notification list")?;

    let rows = stmt
        .query_map(params![limit.clamp(1, 500)], |row| {
            Ok(NotificationRow {
                id: row.get(0)?,
                app_name: row.get(1)?,
                package_name: row.get(2)?,
                sender: row.get(3)?,
                message: row.get(4)?,
                timestamp: row.get(5)?,
                received_at: row.get(6)?,
                status: row.get(7)?,
                priority: row.get(8)?,
                content_hidden: row.get(9)?,
            })
        })
        .context("query notifications")?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("collect notifications")
}

pub fn dismiss_notification(db_path: &Path, id: &str) -> Result<()> {
    mark_status(db_path, id, "READ")
}

pub fn clear_notifications_older_than(db_path: &Path, cutoff_ms: i64) -> Result<usize> {
    let conn = Connection::open(db_path).context("open desktop sqlite database")?;
    conn.execute(
        "DELETE FROM notifications WHERE MIN(timestamp, received_at) < ?1",
        params![cutoff_ms],
    )
    .context("clear old notifications")
}

pub fn clear_all_notifications(db_path: &Path) -> Result<usize> {
    let conn = Connection::open(db_path).context("open desktop sqlite database")?;
    conn.execute("DELETE FROM notifications", [])
        .context("clear all notifications")
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

fn upsert_app_seen(
    conn: &Connection,
    package_name: &str,
    label: &str,
    category: &str,
    seen_at: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO app_rules (
            package_name, label, category, notifications_seen, last_seen_at, updated_at
         ) VALUES (?1, ?2, ?3, 1, ?4, ?5)
         ON CONFLICT(package_name) DO UPDATE SET
            label=excluded.label,
            category=excluded.category,
            notifications_seen=app_rules.notifications_seen + 1,
            last_seen_at=MAX(app_rules.last_seen_at, excluded.last_seen_at)",
        params![package_name, label, category, seen_at, now_millis()],
    )
    .context("upsert app seen")?;
    Ok(())
}

fn get_app_rule(conn: &Connection, package_name: &str) -> Result<AppRuleRow> {
    conn.query_row(
        "SELECT package_name, label, category, notifications_seen, last_seen_at, muted, priority, study_safe, updated_at
         FROM app_rules
         WHERE package_name = ?1",
        params![package_name],
        app_rule_from_row,
    )
    .context("get app rule")
}

fn app_rule_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AppRuleRow> {
    Ok(AppRuleRow {
        package_name: row.get(0)?,
        label: row.get(1)?,
        category: row.get(2)?,
        notifications_seen: row.get(3)?,
        last_seen_at: row.get(4)?,
        muted: row.get(5)?,
        priority: row.get(6)?,
        study_safe: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn categorize_app(package_name: &str, label: &str) -> String {
    let text = format!("{} {}", package_name, label).to_lowercase();
    let category = if contains_any(
        &text,
        &[
            "whatsapp",
            "telegram",
            "signal",
            "messenger",
            "messages",
            "sms",
            "discord",
        ],
    ) {
        "messaging"
    } else if contains_any(&text, &["gmail", "outlook", "mail", "proton"]) {
        "email"
    } else if contains_any(
        &text,
        &["calendar", "meet", "zoom", "teams", "classroom", "canvas"],
    ) {
        "school_work"
    } else if contains_any(&text, &["bank", "paypal", "pay", "wallet", "finance"]) {
        "finance"
    } else if contains_any(
        &text,
        &[
            "instagram",
            "tiktok",
            "snapchat",
            "facebook",
            "twitter",
            "x.",
            "reddit",
        ],
    ) {
        "social"
    } else if contains_any(&text, &["amazon", "shop", "store", "ebay", "walmart"]) {
        "shopping"
    } else if contains_any(&text, &["youtube", "spotify", "netflix", "music", "video"]) {
        "media"
    } else if contains_any(&text, &["android", "system", "settings", "google play"]) {
        "system"
    } else {
        "other"
    };
    category.to_string()
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
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
