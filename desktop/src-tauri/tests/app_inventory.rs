#[allow(dead_code)]
#[path = "../src/db/mod.rs"]
mod db;

use db::store;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::PathBuf;

struct TestDb(PathBuf);

impl TestDb {
    fn new() -> Self {
        let db =
            Self(std::env::temp_dir().join(format!("inventory-{}.sqlite", uuid::Uuid::new_v4())));
        store::init(&db.0).unwrap();
        db
    }

    fn rows(&self) -> Value {
        serde_json::to_value(store::list_app_rules(&self.0).unwrap()).unwrap()
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn snapshot_hides_absent_apps_and_restores_preferences_on_reinstall() {
    let db = TestDb::new();
    store::save_app_inventory(
        &db.0,
        &json!({"apps": [
            {"packageName": "com.old", "iconDataUrl": "data:old", "notificationsSeen": 8},
            {"packageName": "com.kept"}
        ]}),
    )
    .unwrap();
    for flag in ["muted", "priority", "study_safe"] {
        store::set_app_rule_flag(&db.0, "com.old", flag, true).unwrap();
    }
    let rows = store::save_app_inventory(
        &db.0,
        &json!({"apps": [
            {"packageName": "com.kept", "label": "Renamed"}, {"packageName": "com.new"}
        ]}),
    )
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert!(!rows.iter().any(|r| r.package_name == "com.old"));
    let envelope: Value =
        serde_json::from_str(&store::rules_update_envelope(&db.0).unwrap()).unwrap();
    assert_eq!(envelope["payload"]["appRules"].as_array().unwrap().len(), 2);
    store::init(&db.0).unwrap();
    assert_eq!(store::list_app_rules(&db.0).unwrap().len(), 2);
    let rows =
        store::save_app_inventory(&db.0, &json!({"apps": [{"packageName": "com.old"}]})).unwrap();
    assert_eq!(rows.len(), 1);
    let old = &rows[0];
    assert_eq!((old.muted, old.priority, old.study_safe), (1, 1, 1));
    assert_eq!(old.notifications_seen, 8);
    assert_eq!(old.icon_data_url, "data:old");
    let envelope: Value =
        serde_json::from_str(&store::rules_update_envelope(&db.0).unwrap()).unwrap();
    assert_eq!(
        envelope["payload"]["appRules"],
        json!([{
            "packageName": "com.old", "muted": true, "priority": true, "studySafe": true
        }])
    );
}

#[test]
fn explicit_empty_snapshot_hides_inventory_without_deleting_history() {
    let db = TestDb::new();
    store::upsert_notification(&db.0, &json!({"id": "n1", "packageName": "com.old"})).unwrap();
    assert!(store::save_app_inventory(&db.0, &json!({"apps": []}))
        .unwrap()
        .is_empty());
    assert_eq!(store::list_notifications(&db.0, 10).unwrap().len(), 1);
    // A queued notification must not resurrect an app excluded by the snapshot.
    store::upsert_notification(&db.0, &json!({"id": "n2", "packageName": "com.old"})).unwrap();
    assert!(store::list_app_rules(&db.0).unwrap().is_empty());
}

#[test]
fn notifications_cannot_add_unknown_apps_after_an_authoritative_snapshot() {
    let db = TestDb::new();
    store::upsert_notification(&db.0, &json!({"id": "before", "packageName": "com.legacy"}))
        .unwrap();
    assert_eq!(store::list_app_rules(&db.0).unwrap().len(), 1);
    store::save_app_inventory(&db.0, &json!({"apps": []})).unwrap();
    store::init(&db.0).unwrap();
    store::upsert_notification(
        &db.0,
        &json!({"id": "after", "packageName": "com.never.seen"}),
    )
    .unwrap();
    assert!(store::list_app_rules(&db.0).unwrap().is_empty());
    assert_eq!(store::list_notifications(&db.0, 10).unwrap().len(), 2);
    let rows =
        store::save_app_inventory(&db.0, &json!({"apps": [{"packageName": "com.never.seen"}]}))
            .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].notifications_seen, 1);
}

#[test]
fn invalid_snapshots_are_rejected_without_partial_updates() {
    let db = TestDb::new();
    store::save_app_inventory(
        &db.0,
        &json!({"apps": [{"packageName": "com.kept", "label": "Original"}]}),
    )
    .unwrap();
    let before = db.rows();
    let mut invalid = vec![
        json!(null),
        json!({}),
        json!({"apps": null}),
        json!({"apps": {}}),
    ];
    for bad in [
        json!(null),
        json!({}),
        json!({"packageName": ""}),
        json!({"packageName": " "}),
        json!({"packageName": 3}),
        json!({"packageName": "com.kept"}),
        json!({"packageName": "com.bad", "label": 3}),
        json!({"packageName": "com.bad", "category": false}),
        json!({"packageName": "com.bad", "iconDataUrl": {}}),
        json!({"packageName": "com.bad", "notificationsSeen": -1}),
        json!({"packageName": "com.bad", "lastSeenAt": "wrong"}),
    ] {
        invalid.push(json!({"apps": [{"packageName": "com.kept", "label": "Changed"}, bad]}));
    }
    for payload in invalid {
        assert!(
            store::save_app_inventory(&db.0, &payload).is_err(),
            "accepted {payload}"
        );
        assert_eq!(db.rows(), before, "modified rows for {payload}");
    }
}

#[test]
fn database_error_rolls_back_the_whole_snapshot() {
    let db = TestDb::new();
    store::upsert_notification(
        &db.0,
        &json!({"id": "original", "packageName": "com.kept", "appName": "Original"}),
    )
    .unwrap();
    let before = db.rows();
    Connection::open(&db.0)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_inventory BEFORE INSERT ON app_rules
         WHEN NEW.package_name = 'com.fail' BEGIN SELECT RAISE(ABORT, 'test failure'); END;",
        )
        .unwrap();
    assert!(store::save_app_inventory(
        &db.0,
        &json!({"apps": [
            {"packageName": "com.kept", "label": "Changed"}, {"packageName": "com.fail"}
        ]})
    )
    .is_err());
    assert_eq!(db.rows(), before);
    assert_eq!(
        store::get_setting(&db.0, "app_inventory_received").unwrap(),
        None
    );
}

#[test]
fn retained_apps_refresh_metadata_without_resetting_rules_or_counters() {
    let db = TestDb::new();
    store::save_app_inventory(
        &db.0,
        &json!({"apps": [{
            "packageName": "com.kept", "label": "Old", "category": "other",
            "iconDataUrl": "data:old", "notificationsSeen": 9, "lastSeenAt": 100
        }]}),
    )
    .unwrap();
    store::set_app_rule_flag(&db.0, "com.kept", "priority", true).unwrap();
    let rows = store::save_app_inventory(
        &db.0,
        &json!({"apps": [{
            "packageName": "com.kept", "label": "New", "category": "learning",
            "iconDataUrl": null, "notificationsSeen": 0, "lastSeenAt": 0
        }]}),
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(
        (&*row.label, &*row.category, &*row.icon_data_url),
        ("New", "learning", "data:old")
    );
    assert_eq!(
        (row.notifications_seen, row.last_seen_at, row.priority),
        (9, 100, 1)
    );
}

#[test]
fn legacy_database_migrates_without_hiding_existing_rules() {
    let path = std::env::temp_dir().join(format!("inventory-{}.sqlite", uuid::Uuid::new_v4()));
    let db = TestDb(path);
    let conn = Connection::open(&db.0).unwrap();
    conn.execute_batch(include_str!("../migrations/001_initial.sql"))
        .unwrap();
    conn.execute(
        "INSERT INTO app_rules(package_name, label, muted) VALUES ('com.legacy', 'Legacy', 1)",
        [],
    )
    .unwrap();
    drop(conn);
    store::init(&db.0).unwrap();
    store::init(&db.0).unwrap();
    let rows = store::list_app_rules(&db.0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].muted, 1);
    assert!(store::save_app_inventory(&db.0, &json!({"apps": []}))
        .unwrap()
        .is_empty());
}
