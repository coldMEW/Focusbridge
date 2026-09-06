#[allow(dead_code)]
#[path = "../src/db/mod.rs"]
mod db;

use db::encrypted::{self, Database, MigrationStage};
use db::store;
use rusqlite::Connection;
use serde_json::json;
use std::path::{Path, PathBuf};

const KEY: [u8; 32] = [0x71; 32];
const SECRET: &str = "fixture-only-sensitive-message-76fe180b";

struct Fixture {
    database: Option<Database>,
    directory: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            database: None,
            directory: tempfile::tempdir().unwrap(),
        }
    }

    fn path(&self) -> PathBuf {
        self.directory.path().join("fixture.db")
    }

    fn start(&mut self) {
        self.database = Some(encrypted::initialize_with_test_key(&self.path(), KEY).unwrap());
        store::init(&self.path()).unwrap();
    }

    fn stop(&mut self) {
        self.database = None;
    }
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    name.into()
}

fn legacy(path: &Path) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(include_str!("../migrations/001_initial.sql"))
        .unwrap();
    conn.execute_batch(
        "PRAGMA user_version = 17;
         PRAGMA application_id = 1178747986;
         CREATE TABLE duplicate_rows(value BLOB);
         INSERT INTO duplicate_rows VALUES (x'000102'), (x'000102');
         CREATE TRIGGER retained_trigger AFTER INSERT ON duplicate_rows
         BEGIN UPDATE settings SET value = 'trigger-ran' WHERE key = 'trigger'; END;
         INSERT INTO settings VALUES ('trigger', 'not-yet');
         INSERT INTO paired_devices(device_name, device_id, pairing_key, created_at)
         VALUES ('Fixture phone', 'fixture-id', 'fixture-pairing-secret', 1);
         INSERT INTO app_rules(package_name, label, muted) VALUES ('com.fixture', 'Fixture', 1);",
    )
    .unwrap();
    conn.execute("INSERT INTO settings VALUES ('secret', ?1)", [SECRET])
        .unwrap();
}

#[test]
fn whole_database_and_sidecars_hide_content_from_unkeyed_sqlite() {
    let mut fixture = Fixture::new();
    fixture.start();
    let path = fixture.path();
    store::set_setting(&path, "secret", SECRET).unwrap();
    store::save_pairing(&path, "phone", "id", SECRET, "endpoint", "fingerprint").unwrap();
    store::upsert_notification(&path, &json!({"id": "n", "message": SECRET})).unwrap();
    let conn = encrypted::open(&path).unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
    store::set_setting(&path, "wal-secret", SECRET).unwrap();
    for entry in std::fs::read_dir(fixture.directory.path()).unwrap() {
        let file = entry.unwrap().path();
        if file
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with(".access.lock")
        {
            continue;
        }
        let bytes = std::fs::read(file).unwrap();
        assert!(!bytes
            .windows(SECRET.len())
            .any(|part| part == SECRET.as_bytes()));
    }
    assert_ne!(&std::fs::read(&path).unwrap()[..16], b"SQLite format 3\0");
    // SQLCipher without a key takes the ordinary SQLite decoding path.
    let unkeyed = Connection::open(&path).unwrap();
    assert!(unkeyed
        .query_row("SELECT value FROM settings", [], |row| row
            .get::<_, String>(0))
        .is_err());
    drop(unkeyed);
    drop(conn);
    fixture.stop();
    fixture.start();
    assert_eq!(
        store::get_setting(&path, "secret").unwrap().as_deref(),
        Some(SECRET)
    );
}

#[test]
fn wrong_key_and_missing_runtime_key_cannot_modify_or_reset_database() {
    let mut fixture = Fixture::new();
    fixture.start();
    let path = fixture.path();
    store::set_setting(&path, "secret", SECRET).unwrap();
    fixture.stop();
    let before = std::fs::read(&path).unwrap();
    assert!(encrypted::initialize_with_test_key(&path, [0x72; 32]).is_err());
    assert!(store::init(&path).is_err());
    assert!(store::set_setting(&path, "secret", "overwrite").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    fixture.start();
    assert_eq!(
        store::get_setting(&path, "secret").unwrap().as_deref(),
        Some(SECRET)
    );
}

#[test]
fn legacy_export_preserves_schema_data_metadata_and_rules() {
    let mut fixture = Fixture::new();
    let path = fixture.path();
    legacy(&path);
    fixture.start();
    assert_eq!(
        store::get_setting(&path, "secret").unwrap().as_deref(),
        Some(SECRET)
    );
    assert_eq!(
        store::list_paired_devices(&path).unwrap()[0].pairing_key,
        "fixture-pairing-secret"
    );
    assert_eq!(store::list_app_rules(&path).unwrap()[0].muted, 1);
    let conn = encrypted::open(&path).unwrap();
    assert_eq!(
        conn.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        17
    );
    assert_eq!(
        conn.query_row("PRAGMA application_id", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        1178747986
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM duplicate_rows", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
    conn.execute("INSERT INTO duplicate_rows VALUES (NULL)", [])
        .unwrap();
    assert_eq!(
        store::get_setting(&path, "trigger").unwrap().as_deref(),
        Some("trigger-ran")
    );
    assert!(!sibling(&path, ".plaintext-migration").exists());
    assert!(!sibling(&path, ".encrypting").exists());
}

#[test]
fn interrupted_migrations_recover_without_losing_the_original() {
    for stage in [
        MigrationStage::BeforeExport,
        MigrationStage::AfterExport,
        MigrationStage::AfterValidation,
        MigrationStage::AfterBackupRename,
        MigrationStage::AfterInstall,
        MigrationStage::BeforeCleanup,
    ] {
        let mut fixture = Fixture::new();
        let path = fixture.path();
        legacy(&path);
        assert!(encrypted::initialize_with_test_fault(&path, KEY, stage).is_err());
        assert!(path.exists() || sibling(&path, ".plaintext-migration").exists());
        fixture.start();
        assert_eq!(
            store::get_setting(&path, "secret").unwrap().as_deref(),
            Some(SECRET),
            "{stage:?}"
        );
    }
}

#[test]
fn process_crashes_at_publication_boundaries_recover() {
    for stage in [
        "AfterExport",
        "AfterBackupRename",
        "AfterInstall",
        "BeforeCleanup",
    ] {
        let mut fixture = Fixture::new();
        let path = fixture.path();
        legacy(&path);
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "migration_crash_child",
                "--ignored",
                "--nocapture",
            ])
            .env("FOCUSBRIDGE_TEST_DB", &path)
            .env("FOCUSBRIDGE_TEST_CRASH_STAGE", stage)
            .status()
            .unwrap();
        assert_eq!(result.code(), Some(73), "child did not reach {stage}");
        fixture.start();
        assert_eq!(
            store::get_setting(&path, "secret").unwrap().as_deref(),
            Some(SECRET)
        );
    }
}

#[test]
#[ignore = "isolated subprocess helper, invoked by process_crashes_at_publication_boundaries_recover"]
fn migration_crash_child() {
    let path =
        PathBuf::from(std::env::var_os("FOCUSBRIDGE_TEST_DB").expect("fixture path is required"));
    let target = std::env::var("FOCUSBRIDGE_TEST_CRASH_STAGE").unwrap();
    let _ = encrypted::initialize_with_test_hook(&path, KEY, |stage| {
        if format!("{stage:?}") == target {
            std::process::exit(73);
        }
        Ok(())
    })
    .unwrap();
    panic!("requested crash boundary was not reached");
}

#[test]
fn committed_legacy_wal_is_recovered_before_export() {
    let mut fixture = Fixture::new();
    let path = fixture.path();
    legacy(&path);
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "legacy_wal_child", "--ignored", "--nocapture"])
        .env("FOCUSBRIDGE_TEST_DB", &path)
        .status()
        .unwrap();
    assert_eq!(result.code(), Some(74));
    assert!(std::fs::metadata(sibling(&path, "-wal")).unwrap().len() > 32);
    fixture.start();
    assert_eq!(
        store::get_setting(&path, "wal-only").unwrap().as_deref(),
        Some(SECRET)
    );
    assert!(!sibling(&path, "-wal").exists());
}

#[test]
#[ignore = "isolated subprocess helper, invoked by committed_legacy_wal_is_recovered_before_export"]
fn legacy_wal_child() {
    let path =
        PathBuf::from(std::env::var_os("FOCUSBRIDGE_TEST_DB").expect("fixture path is required"));
    let conn = Connection::open(path).unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
        .unwrap();
    conn.execute("INSERT INTO settings VALUES ('wal-only', ?1)", [SECRET])
        .unwrap();
    std::process::exit(74);
}

#[test]
fn independent_sqlite_cli_cannot_read_encrypted_database() {
    let mut fixture = Fixture::new();
    fixture.start();
    store::set_setting(&fixture.path(), "secret", SECRET).unwrap();
    fixture.stop();
    let executable =
        std::env::var_os("FOCUSBRIDGE_SQLITE3_TEST_BIN").unwrap_or_else(|| "sqlite3".into());
    let control = std::process::Command::new(&executable)
        .args([
            ":memory:",
            "SELECT sqlite_version(); PRAGMA cipher_version;",
        ])
        .output()
        .expect("ordinary SQLite CLI required: set FOCUSBRIDGE_SQLITE3_TEST_BIN");
    assert!(control.status.success());
    assert_eq!(
        String::from_utf8(control.stdout).unwrap().lines().count(),
        1,
        "the independent CLI must be ordinary SQLite, not SQLCipher"
    );
    let plaintext = fixture.directory.path().join("ordinary-control.db");
    legacy(&plaintext);
    let control = std::process::Command::new(&executable)
        .arg(&plaintext)
        .arg("SELECT value FROM settings WHERE key='secret';")
        .output()
        .unwrap();
    assert!(control.status.success());
    assert_eq!(String::from_utf8(control.stdout).unwrap().trim(), SECRET);
    let output = std::process::Command::new(&executable)
        .arg(fixture.path())
        .arg("SELECT value FROM settings;")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains(SECRET));
    assert!(String::from_utf8_lossy(&output.stderr).contains("not a database"));
}

#[test]
fn implicit_rowids_survive_export_with_gaps_and_shadowed_aliases() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let source = Connection::open(&path).unwrap();
    source
        .execute_batch(
            "CREATE TABLE items(value TEXT);
         INSERT INTO items(rowid, value) VALUES (-9, 'negative'), (5, 'gap'), (900, 'last');
         CREATE TABLE shadowed(rowid TEXT, value BLOB);
         INSERT INTO shadowed(_rowid_, rowid, value) VALUES (37, 'not-an-id', x'0011');
         CREATE TABLE no_rowid(id TEXT PRIMARY KEY, value TEXT) WITHOUT ROWID;
         INSERT INTO no_rowid VALUES ('id', 'value');",
        )
        .unwrap();
    drop(source);
    let _database = encrypted::initialize_with_test_key(&path, KEY).unwrap();
    let conn = encrypted::open(&path).unwrap();
    let rows = conn
        .prepare("SELECT rowid, value FROM items ORDER BY rowid")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        rows,
        vec![
            (-9, "negative".into()),
            (5, "gap".into()),
            (900, "last".into())
        ]
    );
    assert_eq!(
        conn.query_row("SELECT _rowid_ FROM shadowed", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        37
    );
}

#[test]
fn rowid_repair_does_not_fire_triggers_or_reset_autoincrement() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let source = Connection::open(&path).unwrap();
    source.execute_batch(
        "CREATE TABLE items(id INTEGER PRIMARY KEY AUTOINCREMENT, value TEXT);
         INSERT INTO items(id, value) VALUES (7, 'retained'), (999, 'deleted');
         DELETE FROM items WHERE id = 999;
         CREATE TABLE audit(events INTEGER);
         INSERT INTO audit(rowid, events) VALUES (43, 0);
         CREATE TRIGGER on_insert AFTER INSERT ON items BEGIN UPDATE audit SET events = events + 1; END;
         CREATE TRIGGER on_delete AFTER DELETE ON items BEGIN UPDATE audit SET events = events + 1; END;"
    ).unwrap();
    drop(source);
    let _database = encrypted::initialize_with_test_key(&path, KEY).unwrap();
    let conn = encrypted::open(&path).unwrap();
    assert_eq!(
        conn.query_row("SELECT events FROM audit", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row("SELECT rowid FROM audit", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        43
    );
    conn.execute("INSERT INTO items(value) VALUES ('new')", [])
        .unwrap();
    assert_eq!(conn.last_insert_rowid(), 1000);
    assert_eq!(
        conn.query_row("SELECT events FROM audit", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn unaddressable_implicit_rowid_fails_closed_without_modifying_legacy_data() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let source = Connection::open(&path).unwrap();
    source
        .execute_batch(
            "CREATE TABLE fully_shadowed(rowid TEXT, _rowid_ TEXT, oid TEXT);
         INSERT INTO fully_shadowed VALUES ('a', 'b', 'c');",
        )
        .unwrap();
    drop(source);
    let before = std::fs::read(&path).unwrap();
    assert!(encrypted::initialize_with_test_key(&path, KEY).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn corrupt_export_and_wrong_recovery_key_preserve_all_recovery_files() {
    for corrupt in [false, true] {
        let fixture = Fixture::new();
        let path = fixture.path();
        legacy(&path);
        assert!(
            encrypted::initialize_with_test_fault(&path, KEY, MigrationStage::AfterExport).is_err()
        );
        let candidate = sibling(&path, ".encrypting");
        if corrupt {
            std::fs::write(&candidate, b"interrupted-or-corrupt-ciphertext").unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        let staged = std::fs::read(&candidate).unwrap();
        let key = if corrupt { KEY } else { [0x73; 32] };
        assert!(encrypted::initialize_with_test_key(&path, key).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::read(&candidate).unwrap(), staged);
    }
}

#[test]
fn changes_by_a_legacy_writer_are_not_silently_discarded_on_recovery() {
    let fixture = Fixture::new();
    let path = fixture.path();
    legacy(&path);
    assert!(
        encrypted::initialize_with_test_fault(&path, KEY, MigrationStage::AfterExport).is_err()
    );
    Connection::open(&path)
        .unwrap()
        .execute("INSERT INTO settings VALUES ('late', 'must-survive')", [])
        .unwrap();
    assert!(encrypted::initialize_with_test_key(&path, KEY).is_err());
    let late: String = Connection::open(&path)
        .unwrap()
        .query_row("SELECT value FROM settings WHERE key='late'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(late, "must-survive");
    assert!(sibling(&path, ".encrypting").exists());
}

#[test]
fn rowid_only_legacy_change_is_detected_during_recovery() {
    let fixture = Fixture::new();
    let path = fixture.path();
    legacy(&path);
    assert!(
        encrypted::initialize_with_test_fault(&path, KEY, MigrationStage::AfterExport).is_err()
    );
    Connection::open(&path)
        .unwrap()
        .execute("UPDATE duplicate_rows SET rowid = rowid + 100", [])
        .unwrap();
    assert!(encrypted::initialize_with_test_key(&path, KEY).is_err());
    assert!(sibling(&path, ".encrypting").exists());
    assert_eq!(
        Connection::open(&path)
            .unwrap()
            .query_row("SELECT min(rowid) FROM duplicate_rows", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        101
    );
}

#[test]
fn truncated_database_and_orphan_journals_are_never_reset() {
    for suffix in [
        "",
        "-wal",
        "-journal",
        ".encrypting",
        ".plaintext-migration",
    ] {
        let fixture = Fixture::new();
        let path = fixture.path();
        let artifact = sibling(&path, suffix);
        std::fs::write(&artifact, b"retain-for-recovery").unwrap();
        assert!(
            encrypted::initialize_with_test_key(&path, KEY).is_err(),
            "{suffix}"
        );
        assert_eq!(std::fs::read(&artifact).unwrap(), b"retain-for-recovery");
        if !suffix.is_empty() {
            assert!(!path.exists());
        }
    }
}

#[cfg(windows)]
#[test]
fn lost_dpapi_key_during_migration_preserves_recovery_copies() {
    let fixture = Fixture::new();
    let path = fixture.path();
    legacy(&path);
    assert!(
        encrypted::initialize_with_test_fault(&path, KEY, MigrationStage::AfterBackupRename)
            .is_err()
    );
    let backup = sibling(&path, ".plaintext-migration");
    let staged = sibling(&path, ".encrypting");
    let original = std::fs::read(&backup).unwrap();
    let candidate = std::fs::read(&staged).unwrap();
    assert!(encrypted::initialize(&path).is_err());
    assert!(!db::database_key::key_path(&path).exists());
    assert_eq!(std::fs::read(&backup).unwrap(), original);
    assert_eq!(std::fs::read(&staged).unwrap(), candidate);
}

#[test]
fn background_storage_does_not_depend_on_the_ui_lock() {
    let mut fixture = Fixture::new();
    fixture.start();
    let path = fixture.path();
    // No UI login or unlock is performed; the OS-owned database key is independent.
    std::thread::spawn(move || {
        store::upsert_notification(&path, &json!({"id": "locked-ui", "message": SECRET})).unwrap();
        assert!(store::notification_exists(&path, "locked-ui").unwrap());
    })
    .join()
    .unwrap();
}

#[test]
fn missing_database_is_not_recreated_by_a_store_operation() {
    let mut fixture = Fixture::new();
    fixture.start();
    let path = fixture.path();
    std::fs::rename(&path, sibling(&path, ".held-by-test")).unwrap();
    assert!(store::set_setting(&path, "secret", SECRET).is_err());
    assert!(!path.exists());
}

#[test]
fn encrypted_wal_survives_process_death_and_wrong_key_attempt() {
    let mut fixture = Fixture::new();
    fixture.start();
    fixture.stop();
    let path = fixture.path();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "encrypted_wal_child", "--ignored", "--nocapture"])
        .env("FOCUSBRIDGE_TEST_DB", &path)
        .status()
        .unwrap();
    assert_eq!(result.code(), Some(75));
    let wal = std::fs::read(sibling(&path, "-wal")).unwrap();
    let main = std::fs::read(&path).unwrap();
    assert!(wal.len() > 32);
    assert!(!wal
        .windows(SECRET.len())
        .any(|bytes| bytes == SECRET.as_bytes()));
    // A rejected key must not rewrite a single byte: the owner may still be
    // recovering this file, and SQLite would otherwise checkpoint the ciphertext
    // write-ahead log into the main database as the failed connection is dropped.
    assert!(encrypted::initialize_with_test_key(&path, [0x22; 32]).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), main);
    assert_eq!(std::fs::read(sibling(&path, "-wal")).unwrap(), wal);
    fixture.start();
    assert_eq!(
        store::get_setting(&path, "wal-only").unwrap().as_deref(),
        Some(SECRET)
    );
}

#[test]
#[ignore = "subprocess helper invoked by encrypted_wal_survives_process_death_and_wrong_key_attempt"]
fn encrypted_wal_child() {
    let path = PathBuf::from(std::env::var_os("FOCUSBRIDGE_TEST_DB").unwrap());
    let _database = encrypted::initialize_with_test_key(&path, KEY).unwrap();
    let conn = encrypted::open(&path).unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
        .unwrap();
    conn.execute("INSERT INTO settings VALUES ('wal-only', ?1)", [SECRET])
        .unwrap();
    std::process::exit(75);
}

#[test]
fn process_lock_and_connection_guard_block_duplicate_initialization() {
    let mut fixture = Fixture::new();
    fixture.start();
    let path = fixture.path();
    let conn = encrypted::open(&path).unwrap();
    fixture.stop();
    assert!(encrypted::initialize_with_test_key(&path, KEY).is_err());
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "locked_database_child",
            "--ignored",
            "--nocapture",
        ])
        .env("FOCUSBRIDGE_TEST_DB", &path)
        .status()
        .unwrap();
    assert!(child.success());
    drop(conn);
    fixture.start();
}

#[test]
#[ignore = "subprocess helper invoked by process_lock_and_connection_guard_block_duplicate_initialization"]
fn locked_database_child() {
    let path = PathBuf::from(std::env::var_os("FOCUSBRIDGE_TEST_DB").unwrap());
    assert!(encrypted::initialize_with_test_key(&path, KEY).is_err());
}

#[cfg(windows)]
#[test]
fn blocked_publication_retains_original_and_verified_candidate_for_retry() {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};
    let mut fixture = Fixture::new();
    let path = fixture.path();
    legacy(&path);
    let before = std::fs::read(&path).unwrap();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(&path)
        .unwrap();
    assert!(encrypted::initialize_with_test_key(&path, KEY).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(sibling(&path, ".encrypting").exists());
    drop(held);
    fixture.start();
    assert_eq!(
        store::get_setting(&path, "secret").unwrap().as_deref(),
        Some(SECRET)
    );
}

#[cfg(windows)]
#[test]
fn dpapi_key_cannot_be_replaced_or_used_to_reset_a_missing_database() {
    let mut envelopes = Vec::new();
    let fixtures = [Fixture::new(), Fixture::new()];
    for fixture in &fixtures {
        let _database = encrypted::initialize(&fixture.path()).unwrap();
        store::init(&fixture.path()).unwrap();
        envelopes.push(std::fs::read(db::database_key::key_path(&fixture.path())).unwrap());
    }
    assert_ne!(envelopes[0], envelopes[1]);
    let path = fixtures[0].path();
    let key_path = db::database_key::key_path(&path);
    let before = std::fs::read(&path).unwrap();
    std::fs::write(&key_path, &envelopes[1]).unwrap();
    assert!(encrypted::initialize(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::write(&key_path, &envelopes[0]).unwrap();
    std::fs::rename(&path, sibling(&path, ".held-by-test")).unwrap();
    assert!(encrypted::initialize(&path).is_err());
    assert!(!path.exists());
    assert_eq!(std::fs::read(&key_path).unwrap(), envelopes[0]);
}

#[cfg(windows)]
#[test]
fn dpapi_key_is_wrapped_and_missing_or_corrupt_key_fails_closed() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let database = encrypted::initialize(&path).unwrap();
    store::init(&path).unwrap();
    store::set_setting(&path, "secret", SECRET).unwrap();
    let key_path = db::database_key::key_path(&path);
    let wrapped = std::fs::read(&key_path).unwrap();
    assert!(wrapped.len() > 32);
    drop(database);
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&key_path).unwrap();
    assert!(encrypted::initialize(&path).is_err());
    assert!(!key_path.exists());
    std::fs::write(&key_path, b"invalid-key-envelope").unwrap();
    assert!(encrypted::initialize(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    std::fs::write(&key_path, wrapped).unwrap();
    let _database = encrypted::initialize(&path).unwrap();
    assert_eq!(
        store::get_setting(&path, "secret").unwrap().as_deref(),
        Some(SECRET)
    );
}
