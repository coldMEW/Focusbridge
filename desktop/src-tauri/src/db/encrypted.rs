//! The only production connection factory, including the narrowly scoped
//! plaintext-to-SQLCipher export. Recovery never overwrites a database or key.

use super::database_key::{self, DatabaseKey};
use anyhow::{ensure, Context, Result};
use fs2::FileExt;
use rusqlite::{config::DbConfig, ffi, types::ValueRef, Connection, OpenFlags};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

const STAGED: &str = ".encrypting";
const BACKUP: &str = ".plaintext-migration";
const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";

struct DatabaseContext {
    key: DatabaseKey,
    _process_lock: File,
}

/// Keep this guard alive for the application lifetime, independently of UI lock.
/// It also keeps other upgraded processes from migrating the same database.
pub struct Database {
    _context: Arc<DatabaseContext>,
}

pub struct EncryptedConnection {
    connection: Connection,
    _database: Arc<DatabaseContext>,
}

impl Deref for EncryptedConnection {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl DerefMut for EncryptedConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

fn databases() -> &'static Mutex<HashMap<PathBuf, Weak<DatabaseContext>>> {
    static DATABASES: OnceLock<Mutex<HashMap<PathBuf, Weak<DatabaseContext>>>> = OnceLock::new();
    DATABASES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn initialize(path: &Path) -> Result<Database> {
    initialize_impl(
        path,
        |allow_create| database_key::load_or_create(path, allow_create),
        |_| Ok(()),
    )
}

/// Ordinary reads/writes must not CREATE, migrate, or generate replacement keys.
pub fn open(path: &Path) -> Result<EncryptedConnection> {
    let path = normalized_path(path)?;
    let database = databases()
        .lock()
        .map_err(|_| anyhow::anyhow!("database registry lock poisoned"))?
        .get(&path)
        .and_then(Weak::upgrade)
        .context("database has not been securely initialized; refusing an unkeyed connection")?;
    ensure!(kind(&path)? == FileKind::Encrypted, "encrypted database is missing or was replaced; refusing to create or migrate it during a store operation");
    let connection = connection(&path, Some(&database.key))?;
    Ok(EncryptedConnection {
        connection,
        _database: database,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationStage {
    BeforeExport,
    AfterExport,
    AfterValidation,
    AfterBackupRename,
    AfterInstall,
    BeforeCleanup,
}

#[cfg(test)]
pub fn initialize_with_test_key(path: &Path, key: [u8; 32]) -> Result<Database> {
    initialize_impl(path, |_| Ok(DatabaseKey::for_test(key)), |_| Ok(()))
}

#[cfg(test)]
pub fn initialize_with_test_fault(
    path: &Path,
    key: [u8; 32],
    fault: MigrationStage,
) -> Result<Database> {
    initialize_impl(
        path,
        |_| Ok(DatabaseKey::for_test(key)),
        |stage| {
            ensure!(
                stage != fault,
                "injected migration interruption at {stage:?}"
            );
            Ok(())
        },
    )
}

#[cfg(test)]
pub fn initialize_with_test_hook(
    path: &Path,
    key: [u8; 32],
    hook: impl Fn(MigrationStage) -> Result<()>,
) -> Result<Database> {
    initialize_impl(path, |_| Ok(DatabaseKey::for_test(key)), hook)
}

fn initialize_impl(
    path: &Path,
    load_key: impl FnOnce(bool) -> Result<DatabaseKey>,
    fault: impl Fn(MigrationStage) -> Result<()>,
) -> Result<Database> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .context("database must have an explicit parent directory")?;
    fs::create_dir_all(parent).context("create database directory")?;
    let path = normalized_path(path)?;
    let mut registry = databases()
        .lock()
        .map_err(|_| anyhow::anyhow!("database registry lock poisoned"))?;
    registry.retain(|_, entry| entry.strong_count() != 0);
    ensure!(
        !registry.contains_key(&path),
        "database is already initialized in this process"
    );
    let lock_path = sibling(&path, ".access.lock");
    regular_file_exists(&lock_path)?;
    let process_lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;
    process_lock
        .try_lock_exclusive()
        .context("database is in use by another process; close it before upgrading")?;

    let main = kind(&path)?;
    let has_staged = regular_file_exists(&sibling(&path, STAGED))?;
    let has_backup = regular_file_exists(&sibling(&path, BACKUP))?;
    let has_key = regular_file_exists(&database_key::key_path(&path))?;
    ensure!(
        main != FileKind::Missing || !has_key || has_staged || has_backup,
        "database is missing but its protected key exists; refusing to create an empty replacement"
    );
    if main == FileKind::Missing {
        ensure_no_sidecars(&path)?;
    }
    let allow_create = main != FileKind::Encrypted && !has_staged && !has_backup;
    let key = load_key(allow_create)?;
    prepare(&path, &key, &fault)?;
    let database = Arc::new(DatabaseContext {
        key,
        _process_lock: process_lock,
    });
    registry.insert(path, Arc::downgrade(&database));
    Ok(Database { _context: database })
}

fn normalized_path(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .context("database has no parent")?
        .canonicalize()
        .context("resolve database parent directory")?;
    Ok(parent.join(path.file_name().context("database has no file name")?))
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    name.into()
}

fn regular_file_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            ensure!(
                metadata.file_type().is_file(),
                "database and recovery artifacts must be regular files, not links or directories"
            );
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).context("inspect database or recovery artifact"),
    }
}

#[derive(PartialEq, Eq)]
enum FileKind {
    Missing,
    Plaintext,
    Encrypted,
}

fn kind(path: &Path) -> Result<FileKind> {
    if !regular_file_exists(path)? {
        return Ok(FileKind::Missing);
    }
    let mut header = [0u8; 16];
    File::open(path)?
        .read_exact(&mut header)
        .context("database is truncated or empty; it will not be reset")?;
    Ok(if &header == SQLITE_HEADER {
        FileKind::Plaintext
    } else {
        FileKind::Encrypted
    })
}

/// `None` is used only for a verified plaintext migration source, never for a
/// store connection. No factory open uses CREATE or SQLite URI interpretation.
fn connection(path: &Path, key: Option<&DatabaseKey>) -> Result<Connection> {
    if key.is_none() {
        ensure!(
            kind(path)? == FileKind::Plaintext,
            "plaintext access is restricted to a recognized legacy migration source"
        );
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .context("open database without creation")?;
    // A connection that never proves the key must not modify the database. SQLite
    // otherwise checkpoints the write-ahead log into the main file when the failed
    // connection is dropped: the frames are ciphertext, so a wrong key does not
    // stop it, and a rejected unlock attempt would silently rewrite pages of a
    // database the owner may still be trying to recover. Re-enabled below once the
    // key and integrity probe has succeeded.
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true)
        .context("disable checkpoint-on-close for the key probe")?;
    let version: String = conn
        .query_row("PRAGMA cipher_version", [], |row| row.get(0))
        .context("SQLCipher is unavailable; refusing ordinary SQLite fallback")?;
    ensure!(
        version.starts_with("4."),
        "unsupported SQLCipher major version"
    );
    if let Some(key) = key {
        let secret = key.sqlcipher_key();
        // The C key API avoids embedding the secret in SQL text, tracing, or
        // SQL-bearing errors. SQLCipher copies it; our transient buffer is wiped.
        let result =
            unsafe { ffi::sqlite3_key(conn.handle(), secret.as_ptr().cast(), secret.len() as i32) };
        ensure!(
            result == ffi::SQLITE_OK,
            "SQLCipher rejected the database key"
        );
        conn.execute_batch("PRAGMA cipher_compatibility = 4; PRAGMA cipher_memory_security = ON;")?;
    }
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.execute_batch("PRAGMA temp_store = MEMORY;")?;
    conn.query_row("SELECT count(*) FROM sqlite_schema", [], |row| {
        row.get::<_, i64>(0)
    })
    .context("database key or database integrity is invalid; no reset was performed")?;
    // The key is proven; restore ordinary write-ahead-log maintenance.
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, false)
        .context("restore checkpoint-on-close")?;
    conn.execute_batch("PRAGMA synchronous = FULL; PRAGMA secure_delete = ON;")?;
    Ok(conn)
}

fn validate(conn: &Connection, encrypted: bool) -> Result<()> {
    if encrypted {
        let mut stmt = conn.prepare("PRAGMA cipher_integrity_check")?;
        ensure!(
            stmt.query([])?.next()?.is_none(),
            "SQLCipher page authentication failed; preserve files for recovery"
        );
    }
    let results = conn
        .prepare("PRAGMA integrity_check")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(
        results == ["ok"],
        "database integrity check failed; preserve files for recovery"
    );
    Ok(())
}

fn ensure_no_sidecars(path: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        ensure!(!regular_file_exists(&sibling(path, suffix))?,
            "database journal sidecars remain; refusing to rename, delete, or ignore possible committed data");
    }
    Ok(())
}

fn legacy_locked(path: &Path) -> Result<Connection> {
    let conn = connection(path, None)?;
    let (busy, _, _): (i64, i64, i64) =
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
    ensure!(
        busy == 0,
        "legacy database WAL is busy; close older applications before migrating"
    );
    let mode: String = conn.query_row("PRAGMA journal_mode = DELETE", [], |row| row.get(0))?;
    ensure!(
        mode.eq_ignore_ascii_case("delete"),
        "cannot checkpoint legacy database into its main file"
    );
    conn.execute_batch("PRAGMA locking_mode = EXCLUSIVE; BEGIN EXCLUSIVE;")?;
    validate(&conn, false)?;
    Ok(conn)
}

fn prepare(
    path: &Path,
    key: &DatabaseKey,
    fault: &impl Fn(MigrationStage) -> Result<()>,
) -> Result<()> {
    let staged = sibling(path, STAGED);
    let backup = sibling(path, BACKUP);
    match kind(path)? {
        FileKind::Missing if !regular_file_exists(&backup)? && !regular_file_exists(&staged)? => {
            create_empty(path)?;
            let conn = connection(path, Some(key))?;
            conn.execute_batch("PRAGMA user_version = 0;")?;
            validate(&conn, true)?;
            conn.close().map_err(|(_, error)| error)?;
            sync_file(path)?;
            sync_parent(path)?;
        }
        FileKind::Plaintext => {
            ensure!(!regular_file_exists(&backup)?, "both a legacy database and its migration backup exist; preserve both for manual recovery");
            let source = legacy_locked(path)?;
            let expected = fingerprint(&source)?;
            fault(MigrationStage::BeforeExport)?;
            if !regular_file_exists(&staged)? {
                export(&source, &staged, key)?;
            } else {
                // An interrupted export is only reusable after a keyed reopen
                // and a complete comparison. Never discard an unverified file.
                source.execute_batch("COMMIT;")?;
            }
            fault(MigrationStage::AfterExport)?;
            verify_export(&staged, key, &expected)?;
            fault(MigrationStage::AfterValidation)?;
            source.close().map_err(|(_, error)| error)?;
            ensure_no_sidecars(path)?;
            sync_file(path)?;
            move_without_replace(path, &backup)?;
            fault(MigrationStage::AfterBackupRename)?;
            // If an older writer left a WAL in the final close/rename window,
            // retain everything instead of pairing it with the encrypted file.
            ensure_no_sidecars(path)?;
            install(path, &staged, &backup, key, fault)?;
        }
        FileKind::Missing => {
            ensure!(kind(&backup)? == FileKind::Plaintext && regular_file_exists(&staged)?,
                "incomplete database migration; original and/or candidate need manual recovery, not an empty database");
            install(path, &staged, &backup, key, fault)?;
        }
        FileKind::Encrypted => {
            let conn = connection(path, Some(key))?;
            validate(&conn, true)?;
            conn.close().map_err(|(_, error)| error)?;
            ensure!(!regular_file_exists(&staged)?, "unexpected encrypted candidate beside the live database; preserving both for recovery");
            if regular_file_exists(&backup)? {
                cleanup_backup(path, &backup, key, fault)?;
            }
        }
    }
    Ok(())
}

fn create_empty(path: &Path) -> Result<()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .context("reserve new database without overwriting any file")?
        .sync_all()?;
    sync_parent(path)
}

fn export(source: &Connection, staged: &Path, key: &DatabaseKey) -> Result<()> {
    create_empty(staged)?;
    let secret = key.sqlcipher_key();
    source
        .execute(
            "ATTACH DATABASE ?1 AS encrypted KEY ?2",
            rusqlite::params![
                staged.to_str().context("migration path is not Unicode")?,
                &*secret
            ],
        )
        .context("attach encrypted migration destination")?;
    source.execute_batch("PRAGMA encrypted.cipher_compatibility = 4;")?;
    // SQLite forbids changing synchronous inside the export transaction. New
    // attachments must already have its safe FULL (or EXTRA) default.
    let synchronous: i64 =
        source.query_row("PRAGMA encrypted.synchronous", [], |row| row.get(0))?;
    ensure!(
        synchronous >= 2,
        "migration destination does not have durable synchronous settings"
    );
    for pragma in ["auto_vacuum", "user_version", "application_id"] {
        let value: i64 =
            source.query_row(&format!("PRAGMA main.{pragma}"), [], |row| row.get(0))?;
        source.execute_batch(&format!("PRAGMA encrypted.{pragma} = {value};"))?;
    }
    source
        .query_row("SELECT sqlcipher_export('encrypted')", [], |_| Ok(()))
        .context(
            "export legacy database; original and any partial candidate are retained on failure",
        )?;
    preserve_rowids(source)?;
    source.execute_batch("COMMIT; DETACH DATABASE encrypted;")?;
    sync_file(staged)?;
    sync_parent(staged)
}

fn quoted_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn rowid_alias(conn: &Connection, table: &str) -> Result<Option<&'static str>> {
    let without_rowid: bool = conn.query_row(
        "SELECT wr FROM pragma_table_list WHERE schema = 'main' AND name = ?1",
        [table],
        |row| row.get(0),
    )?;
    if without_rowid {
        return Ok(None);
    }
    let columns = conn
        .prepare("SELECT name FROM pragma_table_xinfo(?1, 'main')")?
        .query_map([table], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ["_rowid_", "rowid", "oid"]
        .into_iter()
        .find(|alias| !columns.iter().any(|name| name.eq_ignore_ascii_case(alias)))
        .map(Some)
        .context("all rowid aliases are shadowed; preserve legacy database for manual migration")
}

fn preserve_rowids(source: &Connection) -> Result<()> {
    // sqlcipher_export uses SELECT *, which can renumber implicit rowids. Copy
    // stored columns again with an explicit rowid, with triggers disabled, then
    // restore sqlite_sequence last so deleted AUTOINCREMENT IDs stay reserved.
    let tables = source
        .prepare(
            "SELECT name FROM main.sqlite_schema WHERE type = 'table' AND rootpage > 0
         ORDER BY name = 'sqlite_sequence', name",
        )?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let triggers = source.db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER)?;
    source.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false)?;
    let result = (|| -> Result<()> {
        for table in tables {
            let Some(alias) = rowid_alias(source, &table)? else {
                continue;
            };
            let mut columns = vec![quoted_identifier(alias)];
            columns.extend(source.prepare(
                "SELECT name FROM pragma_table_xinfo(?1, 'main') WHERE hidden = 0 ORDER BY cid"
            )?.query_map([&table], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?.iter().map(|name| quoted_identifier(name)));
            let table = quoted_identifier(&table);
            let columns = columns.join(", ");
            source
                .execute_batch(&format!(
                    "DELETE FROM encrypted.{table};
                 INSERT INTO encrypted.{table} ({columns}) SELECT {columns} FROM main.{table};"
                ))
                .context("preserve migration row identities without trigger side effects")?;
        }
        Ok(())
    })();
    source.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, triggers)?;
    result
}

fn verify_export(path: &Path, key: &DatabaseKey, expected: &[u8; 32]) -> Result<()> {
    ensure!(
        kind(path)? == FileKind::Encrypted,
        "migration output is not an encrypted database"
    );
    let conn = connection(path, Some(key))?;
    validate(&conn, true)?;
    ensure!(
        &fingerprint(&conn)? == expected,
        "export does not match the legacy schema/data; all copies are retained for recovery"
    );
    conn.close().map_err(|(_, error)| error)?;
    sync_file(path)?;
    ensure_no_sidecars(path)
}

fn install(
    path: &Path,
    staged: &Path,
    backup: &Path,
    key: &DatabaseKey,
    fault: &impl Fn(MigrationStage) -> Result<()>,
) -> Result<()> {
    ensure_no_sidecars(path)?;
    ensure_no_sidecars(backup)?;
    let source = legacy_locked(backup)?;
    verify_export(staged, key, &fingerprint(&source)?)?;
    source.execute_batch("COMMIT;")?;
    source.close().map_err(|(_, error)| error)?;
    move_without_replace(staged, path)?;
    fault(MigrationStage::AfterInstall)?;
    cleanup_backup(path, backup, key, fault)
}

fn cleanup_backup(
    path: &Path,
    backup: &Path,
    key: &DatabaseKey,
    fault: &impl Fn(MigrationStage) -> Result<()>,
) -> Result<()> {
    ensure_no_sidecars(path)?;
    ensure_no_sidecars(backup)?;
    let source = legacy_locked(backup)?;
    // Recompare the original after publication as well: a legacy writer may
    // have committed between closing the source and renaming it on Windows.
    verify_export(path, key, &fingerprint(&source)?)?;
    source.execute_batch("COMMIT;")?;
    source.close().map_err(|(_, error)| error)?;
    fault(MigrationStage::BeforeCleanup)?;
    ensure_no_sidecars(backup)?;
    fs::remove_file(backup).context("remove verified plaintext migration backup")?;
    sync_parent(backup)
}

/// Hash schema and every typed row, including accessible rowids and duplicate
/// multiplicity. Physical page layout may change during export; identities may not.
fn fingerprint(conn: &Connection) -> Result<[u8; 32]> {
    let mut digest = Sha256::new();
    for pragma in ["user_version", "application_id", "auto_vacuum"] {
        let value: i64 = conn.query_row(&format!("PRAGMA {pragma}"), [], |row| row.get(0))?;
        digest.update(value.to_le_bytes());
    }
    let mut schema =
        conn.prepare("SELECT type, name, tbl_name, sql FROM sqlite_schema ORDER BY type, name")?;
    let entries = schema
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (kind, name, table, sql) in entries {
        for text in [
            kind.as_str(),
            name.as_str(),
            table.as_str(),
            sql.as_deref().unwrap_or(""),
        ] {
            hash_bytes(&mut digest, text.as_bytes());
        }
        if kind != "table" {
            continue;
        }
        let identifier = quoted_identifier(&name);
        let projection = match rowid_alias(conn, &name)? {
            Some(alias) => format!("{}, *", quoted_identifier(alias)),
            None => "*".to_string(),
        };
        let mut stmt = conn.prepare(&format!("SELECT {projection} FROM {identifier}"))?;
        let columns = stmt.column_count();
        let mut rows = stmt.query([])?;
        let mut hashes = Vec::<[u8; 32]>::new();
        while let Some(row) = rows.next()? {
            let mut hash = Sha256::new();
            hash.update((columns as u64).to_le_bytes());
            for column in 0..columns {
                match row.get_ref(column)? {
                    ValueRef::Null => hash.update([0]),
                    ValueRef::Integer(value) => {
                        hash.update([1]);
                        hash.update(value.to_le_bytes());
                    }
                    ValueRef::Real(value) => {
                        hash.update([2]);
                        hash.update(value.to_bits().to_le_bytes());
                    }
                    ValueRef::Text(value) => {
                        hash.update([3]);
                        hash_bytes(&mut hash, value);
                    }
                    ValueRef::Blob(value) => {
                        hash.update([4]);
                        hash_bytes(&mut hash, value);
                    }
                }
            }
            hashes.push(hash.finalize().into());
        }
        hashes.sort_unstable();
        digest.update((hashes.len() as u64).to_le_bytes());
        for hash in hashes {
            digest.update(hash);
        }
    }
    Ok(digest.finalize().into())
}

fn hash_bytes(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn sync_file(path: &Path) -> Result<()> {
    OpenOptions::new()
        .write(true)
        .open(path)?
        .sync_all()
        .context("flush database before migration publication")
}

fn sync_parent(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path.parent().context("missing database parent")?)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn move_without_replace(from: &Path, to: &Path) -> Result<()> {
    ensure!(
        !regular_file_exists(to)?,
        "migration destination already exists; refusing to overwrite it"
    );
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};
        let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
        ensure!(
            !from[..from.len() - 1].contains(&0) && !to[..to.len() - 1].contains(&0),
            "invalid migration path"
        );
        // Same-directory move, durable publication, no REPLACE_EXISTING flag.
        if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) } == 0 {
            return Err(std::io::Error::last_os_error()).context(
                "publish database migration without overwrite; close older app processes and retry",
            );
        }
    }
    #[cfg(not(windows))]
    {
        // Production key acquisition rejects these platforms. The no-clobber
        // path permits portable, explicitly keyed fixture tests only.
        fs::hard_link(from, to).context("publish fixture database without overwrite")?;
        sync_parent(to)?;
        fs::remove_file(from)?;
    }
    sync_parent(to)
}
