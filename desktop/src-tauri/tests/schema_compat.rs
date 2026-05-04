use focusbridge_desktop::db;
use rusqlite::Connection;
use serde_json::Value;

#[test]
fn store_init_upgrades_existing_app_rules_without_icon_column() {
    let db_path = std::env::temp_dir().join(format!(
        "focusbridge-schema-compat-{}.db",
        uuid::Uuid::new_v4()
    ));

    {
        let conn = Connection::open(&db_path).expect("open temp sqlite database");
        conn.execute_batch(
            "
            CREATE TABLE app_rules (
                package_name TEXT PRIMARY KEY,
                label TEXT NOT NULL,
                category TEXT NOT NULL DEFAULT 'other',
                notifications_seen INTEGER NOT NULL DEFAULT 0,
                last_seen_at INTEGER NOT NULL DEFAULT 0,
                muted INTEGER NOT NULL DEFAULT 0,
                priority INTEGER NOT NULL DEFAULT 0,
                study_safe INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL DEFAULT 0
            );
            ",
        )
        .expect("create legacy app_rules table");
    }

    db::store::init(&db_path).expect("initialize schema");

    let conn = Connection::open(&db_path).expect("reopen temp sqlite database");
    let columns = conn
        .prepare("PRAGMA table_info(app_rules)")
        .expect("prepare table info")
        .query_map([], |row| row.get::<_, String>(1))
        .expect("query table info")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect columns");

    assert!(columns.iter().any(|column| column == "icon_data_url"));

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn rules_update_envelope_contains_app_and_keyword_rules() {
    let db_path = std::env::temp_dir().join(format!(
        "focusbridge-rules-update-{}.db",
        uuid::Uuid::new_v4()
    ));

    db::store::init(&db_path).expect("initialize schema");
    db::store::set_app_rule_flag(&db_path, "com.social.app", "muted", true)
        .expect("set muted rule");
    db::store::set_app_rule_flag(&db_path, "com.mail.app", "priority", true)
        .expect("set priority rule");
    db::store::set_setting(&db_path, "priority_keywords", "deadline,exam").expect("set keywords");

    let encoded = db::store::rules_update_envelope(&db_path).expect("build rules update");
    let envelope: Value = serde_json::from_str(&encoded).expect("decode rules update");

    assert_eq!(envelope["type"], "RULES_UPDATE");
    let app_rules = envelope["payload"]["appRules"]
        .as_array()
        .expect("app rules array");
    assert!(app_rules
        .iter()
        .any(|rule| rule["packageName"] == "com.social.app" && rule["muted"] == true));
    assert_eq!(envelope["payload"]["priorityKeywords"][0], "deadline");

    let _ = std::fs::remove_file(db_path);
}
