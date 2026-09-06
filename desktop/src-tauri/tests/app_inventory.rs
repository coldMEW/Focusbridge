#[allow(dead_code)]
#[path = "../src/db/mod.rs"]
mod db;

use db::{encrypted, store};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::PathBuf;

struct TestDb {
    path: PathBuf,
    _database: encrypted::Database,
    _directory: tempfile::TempDir,
}

impl TestDb {
    fn new() -> Self {
        Self::with_legacy(|_| {})
    }

    fn with_legacy(prepare: impl FnOnce(&PathBuf)) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("inventory.sqlite");
        prepare(&path);
        let database = encrypted::initialize_with_test_key(&path, [0x53; 32]).unwrap();
        let db = Self {
            path,
            _database: database,
            _directory: directory,
        };
        store::init(&db.path).unwrap();
        db
    }

    fn rows(&self) -> Value {
        serde_json::to_value(store::list_app_rules(&self.path).unwrap()).unwrap()
    }
}

#[test]
fn snapshot_hides_absent_apps_and_restores_preferences_on_reinstall() {
    let db = TestDb::new();
    store::save_app_inventory(
        &db.path,
        &json!({"apps": [
            {"packageName": "com.old", "iconDataUrl": "data:old", "notificationsSeen": 8},
            {"packageName": "com.kept"}
        ]}),
    )
    .unwrap();
    for flag in ["muted", "priority", "study_safe"] {
        store::set_app_rule_flag(&db.path, "com.old", flag, true).unwrap();
    }
    let rows = store::save_app_inventory(
        &db.path,
        &json!({"apps": [
            {"packageName": "com.kept", "label": "Renamed"}, {"packageName": "com.new"}
        ]}),
    )
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert!(!rows.iter().any(|r| r.package_name == "com.old"));
    let envelope: Value =
        serde_json::from_str(&store::rules_update_envelope(&db.path).unwrap()).unwrap();
    assert_eq!(envelope["payload"]["appRules"].as_array().unwrap().len(), 2);
    store::init(&db.path).unwrap();
    assert_eq!(store::list_app_rules(&db.path).unwrap().len(), 2);
    let rows = store::save_app_inventory(&db.path, &json!({"apps": [{"packageName": "com.old"}]}))
        .unwrap();
    assert_eq!(rows.len(), 1);
    let old = &rows[0];
    assert_eq!((old.muted, old.priority, old.study_safe), (1, 1, 1));
    assert_eq!(old.notifications_seen, 8);
    assert_eq!(old.icon_data_url, "data:old");
    let envelope: Value =
        serde_json::from_str(&store::rules_update_envelope(&db.path).unwrap()).unwrap();
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
    store::upsert_notification(&db.path, &json!({"id": "n1", "packageName": "com.old"})).unwrap();
    assert!(store::save_app_inventory(&db.path, &json!({"apps": []}))
        .unwrap()
        .is_empty());
    assert_eq!(store::list_notifications(&db.path, 10).unwrap().len(), 1);
    // A queued notification must not resurrect an app excluded by the snapshot.
    store::upsert_notification(&db.path, &json!({"id": "n2", "packageName": "com.old"})).unwrap();
    assert!(store::list_app_rules(&db.path).unwrap().is_empty());
}

#[test]
fn notifications_cannot_add_unknown_apps_after_an_authoritative_snapshot() {
    let db = TestDb::new();
    store::upsert_notification(
        &db.path,
        &json!({"id": "before", "packageName": "com.legacy"}),
    )
    .unwrap();
    assert_eq!(store::list_app_rules(&db.path).unwrap().len(), 1);
    store::save_app_inventory(&db.path, &json!({"apps": []})).unwrap();
    store::init(&db.path).unwrap();
    store::upsert_notification(
        &db.path,
        &json!({"id": "after", "packageName": "com.never.seen"}),
    )
    .unwrap();
    assert!(store::list_app_rules(&db.path).unwrap().is_empty());
    assert_eq!(store::list_notifications(&db.path, 10).unwrap().len(), 2);
    let rows = store::save_app_inventory(
        &db.path,
        &json!({"apps": [{"packageName": "com.never.seen"}]}),
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].notifications_seen, 1);
}

#[test]
fn invalid_snapshots_are_rejected_without_partial_updates() {
    let db = TestDb::new();
    store::save_app_inventory(
        &db.path,
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
            store::save_app_inventory(&db.path, &payload).is_err(),
            "accepted {payload}"
        );
        assert_eq!(db.rows(), before, "modified rows for {payload}");
    }
}

#[test]
fn database_error_rolls_back_the_whole_snapshot() {
    let db = TestDb::new();
    store::upsert_notification(
        &db.path,
        &json!({"id": "original", "packageName": "com.kept", "appName": "Original"}),
    )
    .unwrap();
    let before = db.rows();
    encrypted::open(&db.path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_inventory BEFORE INSERT ON app_rules
         WHEN NEW.package_name = 'com.fail' BEGIN SELECT RAISE(ABORT, 'test failure'); END;",
        )
        .unwrap();
    assert!(store::save_app_inventory(
        &db.path,
        &json!({"apps": [
            {"packageName": "com.kept", "label": "Changed"}, {"packageName": "com.fail"}
        ]})
    )
    .is_err());
    assert_eq!(db.rows(), before);
    assert_eq!(
        store::get_setting(&db.path, "app_inventory_received").unwrap(),
        None
    );
}

#[test]
fn retained_apps_refresh_metadata_without_resetting_rules_or_counters() {
    let db = TestDb::new();
    store::save_app_inventory(
        &db.path,
        &json!({"apps": [{
            "packageName": "com.kept", "label": "Old", "category": "other",
            "iconDataUrl": "data:old", "notificationsSeen": 9, "lastSeenAt": 100
        }]}),
    )
    .unwrap();
    store::set_app_rule_flag(&db.path, "com.kept", "priority", true).unwrap();
    let rows = store::save_app_inventory(
        &db.path,
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
    let db = TestDb::with_legacy(|path| {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(include_str!("../migrations/001_initial.sql"))
            .unwrap();
        conn.execute(
            "INSERT INTO app_rules(package_name, label, muted) VALUES ('com.legacy', 'Legacy', 1)",
            [],
        )
        .unwrap();
        drop(conn);
    });
    store::init(&db.path).unwrap();
    store::init(&db.path).unwrap();
    let rows = store::list_app_rules(&db.path).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].muted, 1);
    assert!(store::save_app_inventory(&db.path, &json!({"apps": []}))
        .unwrap()
        .is_empty());
}
