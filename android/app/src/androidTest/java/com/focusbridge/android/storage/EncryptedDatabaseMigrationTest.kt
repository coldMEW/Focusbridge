package com.focusbridge.android.storage

import android.content.Context
import android.content.ContextWrapper
import android.database.sqlite.SQLiteDatabase
import android.os.Build
import android.security.keystore.KeyInfo
import android.system.ErrnoException
import android.system.Os
import android.system.OsConstants
import androidx.room.Room
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.focusbridge.android.data.local.FocusBridgeDatabase
import com.focusbridge.android.data.local.FOCUSBRIDGE_SCHEMA_VERSION
import com.focusbridge.android.di.DatabaseModule
import com.focusbridge.android.security.DatabaseKeyStore
import java.io.File
import java.io.IOException
import java.security.GeneralSecurityException
import java.security.KeyStore
import java.util.UUID
import javax.crypto.SecretKeyFactory
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

/** Disposable emulator only. UUID-scoped fixtures and aliases never use the runtime DB or key. */
@RunWith(AndroidJUnit4::class)
class EncryptedDatabaseMigrationTest {
    private lateinit var context: Context
    private lateinit var root: File
    private lateinit var file: File
    private lateinit var keys: DatabaseKeyStore

    @Before fun setUp() {
        check(Build.FINGERPRINT.contains("generic") || Build.MODEL.contains("sdk", ignoreCase = true)) {
            "Storage migration tests must run on a disposable Android emulator, not a real device"
        }
        val testContext = InstrumentationRegistry.getInstrumentation().context
        val id = UUID.randomUUID().toString()
        // Instrumentation executes as the target UID, not the test APK's distinct UID.
        val cache = InstrumentationRegistry.getInstrumentation().targetContext.cacheDir
        root = File(cache, "encrypted-database-test-$id").apply { check(mkdirs()) }
        context = object : ContextWrapper(testContext) {
            override fun getApplicationContext(): Context = this
            override fun getPackageName(): String = "${testContext.packageName}.isolated.$id"
            override fun getDatabasePath(name: String): File = File(root, name)
            override fun getNoBackupFilesDir(): File = File(root, "keys").apply { mkdirs() }
        }
        file = context.getDatabasePath("fixture.db")
        keys = DatabaseKeyStore(context)
        System.loadLibrary("sqlcipher")
    }

    @After fun tearDown() {
        if (::context.isInitialized) {
            KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
                .deleteEntry(DatabaseKeyStore.alias(context))
        }
        if (::root.isInitialized) check(root.deleteRecursively())
    }

    @Test fun migratesVersionsOneTwoAndThreeWithoutLosingData() {
        for (version in 1..3) {
            val candidate = File(root, "version-$version.db")
            plaintext(candidate, version)
            val passphrase = EncryptedDatabaseMigrator(candidate).prepare(keys)
            try {
                assertFalse(EncryptedDatabaseMigrator.hasPlaintextHeader(candidate))
                encrypted(candidate, passphrase) { db ->
                    assertEquals(version, db.version)
                    assertEquals("private notification", scalar(db, "SELECT message FROM notifications"))
                    assertEquals("pairing-secret", scalar(db, "SELECT pairingKey FROM pairings"))
                    assertEquals("setting-value", scalar(db, "SELECT value FROM config"))
                    assertEquals("45", scalar(db, "SELECT seq FROM sqlite_sequence WHERE name='audit'"))
                    assertEquals("000102FF", scalar(db, "SELECT hex(payload) FROM audit"))
                    assertEquals("private notification", scalar(db, "SELECT message FROM notification_messages"))
                    assertEquals("1", scalar(db, "SELECT count(*) FROM sqlite_master WHERE type='trigger' AND name='audit_changes'"))
                    if (version == 3) assertEquals("1", scalar(db, "SELECT priority FROM app_rules"))
                }
                val room = DatabaseModule.buildEncryptedDatabase(context, candidate.name)
                try {
                    // Room upgrades the opened database to the app's current schema.
                    assertEquals(
                        FOCUSBRIDGE_SCHEMA_VERSION,
                        room.openHelper.writableDatabase.version,
                    )
                    room.openHelper.writableDatabase.query("SELECT endpointCandidates FROM pairings").use {
                        assertTrue(it.moveToFirst())
                        assertEquals(if (version == 1) "" else "candidate", it.getString(0))
                    }
                } finally {
                    room.close()
                }
            } finally {
                passphrase.fill(0)
            }
        }
    }

    @Test fun preservesRoomGeneratedSchemaIdentityAndReopensWithEncryptedFactory() {
        val plain = Room.databaseBuilder(context, FocusBridgeDatabase::class.java, file.name).build()
        val identity: String
        try {
            plain.openHelper.writableDatabase.execSQL("INSERT INTO config VALUES ('setting', 'room-created')")
            identity = plain.openHelper.writableDatabase.query("SELECT identity_hash FROM room_master_table WHERE id=42").use {
                check(it.moveToFirst())
                it.getString(0)
            }
        } finally {
            plain.close()
        }
        repeat(2) {
            val room = DatabaseModule.buildEncryptedDatabase(context, file.name)
            try {
                room.openHelper.writableDatabase.query("SELECT identity_hash FROM room_master_table WHERE id=42").use {
                    assertTrue(it.moveToFirst())
                    assertEquals(identity, it.getString(0))
                }
                room.openHelper.writableDatabase.query("SELECT value FROM config WHERE key='setting'").use {
                    assertTrue(it.moveToFirst())
                    assertEquals("room-created", it.getString(0))
                }
            } finally {
                room.close()
            }
        }
        assertFalse(EncryptedDatabaseMigrator.hasPlaintextHeader(file))
    }

    @Test fun migratesCommittedRowsThatExistOnlyInWal() {
        val live = File(root, "wal-source.db")
        plaintext(live, 3)
        SQLiteDatabase.openDatabase(live.path, null, SQLiteDatabase.OPEN_READWRITE).use { db ->
            check(db.enableWriteAheadLogging())
            db.rawQuery("PRAGMA wal_autocheckpoint=0", null).use { it.moveToFirst() }
            db.execSQL("INSERT INTO config VALUES ('wal-only', 'committed-in-wal')")
            assertTrue(File(live.path + "-wal").length() > 0)
            // Only synthetic fixtures are copied, after commit and while no writer is active.
            live.copyTo(file)
            File(live.path + "-wal").copyTo(File(file.path + "-wal"))
        }
        val passphrase = EncryptedDatabaseMigrator(file).prepare(keys)
        try {
            encrypted(file, passphrase) { db ->
                assertEquals("committed-in-wal", scalar(db, "SELECT value FROM config WHERE key='wal-only'"))
            }
            assertFalse(File(file.path + "-wal").exists())
            assertFalse(File(file.path + "-shm").exists())
        } finally {
            passphrase.fill(0)
        }
    }

    @Test fun rejectsWrongAndEmptyKeysWithoutDeletingOrChangingDatabase() {
        plaintext(file, 3)
        EncryptedDatabaseMigrator(file).prepare(keys).fill(0)
        val before = file.readBytes()
        for (key in listOf(ByteArray(32) { 7 }, ByteArray(0))) {
            assertThrows(android.database.sqlite.SQLiteException::class.java) {
                encrypted(file, key) { scalar(it, "SELECT count(*) FROM sqlite_master") }
            }
            assertArrayEquals(before, file.readBytes())
        }
        assertFalse(file.readBytes().toString(Charsets.ISO_8859_1).contains("private notification"))
    }

    @Test fun migratesTruncatedRollbackJournalWithoutChangingSourceBeforeExport() {
        plaintext(file, 3)
        SQLiteDatabase.openDatabase(file.path, null, SQLiteDatabase.OPEN_READWRITE).use { db ->
            db.rawQuery("PRAGMA journal_mode=TRUNCATE", null).use {
                assertTrue(it.moveToFirst())
                assertEquals("truncate", it.getString(0))
            }
            db.execSQL("INSERT INTO config VALUES ('journal', 'retained')")
        }
        val journal = File(file.path + "-journal")
        assertTrue(journal.exists())
        assertEquals(0L, journal.length())
        val before = file.readBytes()
        val passphrase = EncryptedDatabaseMigrator(file) {
            if (it == EncryptedDatabaseMigrator.Phase.CHECKPOINTED) {
                assertArrayEquals(before, file.readBytes())
                assertFalse(journal.exists())
            }
        }.prepare(keys)
        try {
            encrypted(file, passphrase) {
                assertEquals("retained", scalar(it, "SELECT value FROM config WHERE key='journal'"))
            }
        } finally {
            passphrase.fill(0)
        }
    }

    @Test fun sqlcipherRetiresColdRollbackJournalThroughJournalModeTransition() {
        plaintext(file, 3)
        val journal = File(file.path + "-journal")
        assertTrue(journal.exists())
        val before = file.readBytes()
        net.zetetic.database.sqlcipher.SQLiteDatabase.openDatabase(
            file.path, ByteArray(0), null,
            net.zetetic.database.sqlcipher.SQLiteDatabase.OPEN_READWRITE,
            EncryptedDatabaseMigrator.PRESERVE_ON_CORRUPTION, null,
        ).use { db ->
            assertEquals("delete", scalar(db, "PRAGMA journal_mode"))
            assertTrue(journal.exists())
            assertEquals("persist", scalar(db, "PRAGMA journal_mode=PERSIST"))
            assertEquals("delete", scalar(db, "PRAGMA journal_mode=DELETE"))
            assertFalse(journal.exists())
        }
        assertArrayEquals(before, file.readBytes())
    }

    @Test fun tamperedExportIsRejectedBeforeReplacingPlaintextSource() {
        plaintext(file, 3)
        val before = file.readBytes()
        assertThrows(android.database.sqlite.SQLiteException::class.java) {
            EncryptedDatabaseMigrator(file) {
                if (it == EncryptedDatabaseMigrator.Phase.EXPORTED) {
                    File(file.path + EncryptedDatabaseMigrator.STAGING_SUFFIX).writeBytes(ByteArray(8192) { 5 })
                }
            }.prepare(keys)
        }
        assertArrayEquals(before, file.readBytes())
        assertTrue(EncryptedDatabaseMigrator.hasPlaintextHeader(file))
    }

    @Test fun activeWalReaderPreventsReplacementRatherThanDroppingCommittedRows() {
        plaintext(file, 3)
        // Two SQLite libraries in one process do not share SQLite's POSIX lock bookkeeping.
        // Use SQLCipher for every simultaneously open handle, as the encrypted app does.
        net.zetetic.database.sqlcipher.SQLiteDatabase.openDatabase(
            file.path, ByteArray(0), null, net.zetetic.database.sqlcipher.SQLiteDatabase.OPEN_READWRITE,
            EncryptedDatabaseMigrator.PRESERVE_ON_CORRUPTION, null,
        ).use { writer ->
            check(writer.enableWriteAheadLogging())
            writer.execSQL("INSERT INTO config VALUES ('second', 'retained')")
            encrypted(file, ByteArray(0)) { reader ->
                reader.execSQL("BEGIN")
                try {
                    reader.rawQuery("SELECT * FROM config", null).use { check(it.moveToFirst()) }
                    writer.execSQL("INSERT INTO config VALUES ('third', 'also-retained')")
                    assertEquals("0", scalar(reader, "SELECT count(*) FROM config WHERE key='third'"))
                    assertThrows(Exception::class.java) { EncryptedDatabaseMigrator(file).prepare(keys) }
                    assertTrue(EncryptedDatabaseMigrator.hasPlaintextHeader(file))
                } finally {
                    reader.execSQL("ROLLBACK")
                }
            }
        }
        val passphrase = EncryptedDatabaseMigrator(file).prepare(keys)
        try {
            encrypted(file, passphrase) { db ->
                assertEquals("also-retained", scalar(db, "SELECT value FROM config WHERE key='third'"))
            }
        } finally {
            passphrase.fill(0)
        }
    }

    @Test fun recoversAtEveryMigrationBoundary() {
        for (phase in EncryptedDatabaseMigrator.Phase.values()) {
            val candidate = File(root, "crash-${phase.name}.db")
            plaintext(candidate, 3)
            assertThrows(IOException::class.java) {
                EncryptedDatabaseMigrator(candidate) { reached ->
                    if (reached == phase) throw IOException("simulated interruption")
                }.prepare(keys)
            }
            assertTrue(candidate.exists())
            val passphrase = EncryptedDatabaseMigrator(candidate).prepare(keys)
            try {
                encrypted(candidate, passphrase) { db ->
                    assertEquals("private notification", scalar(db, "SELECT message FROM notifications"))
                }
                assertFalse(File(candidate.path + EncryptedDatabaseMigrator.STAGING_SUFFIX).exists())
                assertFalse(EncryptedDatabaseMigrator.hasPlaintextHeader(candidate))
            } finally {
                passphrase.fill(0)
            }
        }
    }

    @Test fun corruptedStageIsReexportedFromIntactSource() {
        plaintext(file, 3)
        assertThrows(IOException::class.java) {
            EncryptedDatabaseMigrator(file) {
                if (it == EncryptedDatabaseMigrator.Phase.EXPORTED) throw IOException("interrupted")
            }.prepare(keys)
        }
        File(file.path + EncryptedDatabaseMigrator.STAGING_SUFFIX).writeBytes(ByteArray(53))
        EncryptedDatabaseMigrator(file).prepare(keys).fill(0)
        assertFalse(EncryptedDatabaseMigrator.hasPlaintextHeader(file))
    }

    @Test fun refusesMissingSourceInsteadOfPromotingUnprovenStage() {
        plaintext(file, 3)
        assertThrows(IOException::class.java) {
            EncryptedDatabaseMigrator(file) {
                if (it == EncryptedDatabaseMigrator.Phase.EXPORTED) throw IOException("interrupted")
            }.prepare(keys)
        }
        check(file.delete())
        val stage = File(file.path + EncryptedDatabaseMigrator.STAGING_SUFFIX)
        val before = stage.readBytes()
        assertThrows(Exception::class.java) { EncryptedDatabaseMigrator(file).prepare(keys) }
        assertFalse(file.exists())
        assertArrayEquals(before, stage.readBytes())
    }

    @Test fun corruptSourceIsPreservedAndNeverReplaced() {
        file.writeBytes("SQLite format 3\u0000not-a-database".toByteArray())
        val before = file.readBytes()
        assertThrows(Exception::class.java) { EncryptedDatabaseMigrator(file).prepare(keys) }
        assertArrayEquals(before, file.readBytes())
    }

    @Test fun missingWrappedRecordFailsClosedForEncryptedDatabase() {
        plaintext(file, 3)
        EncryptedDatabaseMigrator(file).prepare(keys).fill(0)
        val before = file.readBytes()
        check(File(context.noBackupFilesDir, DatabaseKeyStore.RECORD_NAME).delete())
        assertThrows(GeneralSecurityException::class.java) { EncryptedDatabaseMigrator(file).prepare(keys) }
        assertArrayEquals(before, file.readBytes())
        assertFalse(File(context.noBackupFilesDir, DatabaseKeyStore.RECORD_NAME).exists())
    }

    @Test fun missingKeystoreAliasFailsClosedWithoutReplacingRecord() {
        plaintext(file, 3)
        EncryptedDatabaseMigrator(file).prepare(keys).fill(0)
        val record = File(context.noBackupFilesDir, DatabaseKeyStore.RECORD_NAME)
        val before = record.readBytes()
        KeyStore.getInstance("AndroidKeyStore").apply { load(null) }.deleteEntry(DatabaseKeyStore.alias(context))
        assertThrows(GeneralSecurityException::class.java) { EncryptedDatabaseMigrator(file).prepare(keys) }
        assertArrayEquals(before, record.readBytes())
    }

    @Test fun wrappingKeyIsNonExportableAndDoesNotRequireAuthentication() {
        val first = keys.getOrCreate(false)
        assertArrayEquals(first, DatabaseKeyStore(context).getOrCreate(true))
        val key = DatabaseKeyStore.AndroidWrappingKeys(DatabaseKeyStore.alias(context)).find()!!
        assertNull(key.encoded)
        val info = SecretKeyFactory.getInstance("AES", "AndroidKeyStore").getKeySpec(key, KeyInfo::class.java) as KeyInfo
        assertEquals(256, info.keySize)
        assertFalse(info.isUserAuthenticationRequired)
        first.fill(0)
    }

    @Test fun interruptedKeyRecordCannotBeTreatedAsFirstRun() {
        val pending = File(context.noBackupFilesDir, DatabaseKeyStore.RECORD_NAME + ".pending")
        pending.writeBytes(byteArrayOf(1, 2, 3))
        assertThrows(IOException::class.java) { keys.getOrCreate(false) }
        assertArrayEquals(byteArrayOf(1, 2, 3), pending.readBytes())
        assertNull(DatabaseKeyStore.AndroidWrappingKeys(DatabaseKeyStore.alias(context)).find())
    }

    @Test fun freshDatabaseIsEncryptedAndCanReopenAfterClose() {
        repeat(2) { attempt ->
            val room = DatabaseModule.buildEncryptedDatabase(context, file.name)
            try {
                val db = room.openHelper.writableDatabase
                if (attempt == 0) db.execSQL("INSERT INTO config VALUES ('fresh', 'encrypted-from-creation')")
                db.query("SELECT value FROM config WHERE key='fresh'").use {
                    assertTrue(it.moveToFirst())
                    assertEquals("encrypted-from-creation", it.getString(0))
                }
            } finally {
                room.close()
            }
            assertFalse(EncryptedDatabaseMigrator.hasPlaintextHeader(file))
        }
    }

    @Test fun ordinaryAndroidSqliteCannotReadEncryptedDatabase() {
        plaintext(file, 3)
        EncryptedDatabaseMigrator(file).prepare(keys).fill(0)
        val before = file.readBytes()
        assertThrows(android.database.sqlite.SQLiteException::class.java) {
            SQLiteDatabase.openDatabase(file.path, null, SQLiteDatabase.OPEN_READONLY,
                android.database.DatabaseErrorHandler { throw android.database.sqlite.SQLiteException("Preserve fixture") },
            ).use { db -> db.rawQuery("SELECT * FROM config", null).use { it.moveToFirst() } }
        }
        assertArrayEquals(before, file.readBytes())
    }

    @Test fun exportPermissionFailurePreservesSourceAndRetryRecovers() {
        plaintext(file, 3)
        val before = file.readBytes()
        try {
            assertThrows(android.database.sqlite.SQLiteException::class.java) {
                EncryptedDatabaseMigrator(file) {
                    if (it == EncryptedDatabaseMigrator.Phase.CHECKPOINTED) Os.chmod(root.path, 320) // 0500
                }.prepare(keys)
            }
            assertArrayEquals(before, file.readBytes())
            assertTrue(EncryptedDatabaseMigrator.hasPlaintextHeader(file))
        } finally {
            Os.chmod(root.path, 448) // 0700
        }
        assertRecoveredConfig()
    }

    @Test fun atomicRenamePermissionFailurePreservesSourceAndRetryRecovers() {
        plaintext(file, 3)
        val before = file.readBytes()
        try {
            val failure = assertThrows(ErrnoException::class.java) {
                EncryptedDatabaseMigrator(file) {
                    if (it == EncryptedDatabaseMigrator.Phase.BEFORE_REPLACE) Os.chmod(root.path, 320)
                }.prepare(keys)
            }
            assertEquals(OsConstants.EACCES, failure.errno)
            assertArrayEquals(before, file.readBytes())
            assertTrue(File(file.path + EncryptedDatabaseMigrator.STAGING_SUFFIX).exists())
        } finally {
            Os.chmod(root.path, 448)
        }
        assertRecoveredConfig()
    }

    @Test fun keyRecordPermissionFailureNeverStartsMigrationAndRetryRecovers() {
        plaintext(file, 3)
        val before = file.readBytes()
        val directory = context.noBackupFilesDir
        try {
            Os.chmod(directory.path, 320)
            assertThrows(IOException::class.java) { EncryptedDatabaseMigrator(file).prepare(keys) }
            assertArrayEquals(before, file.readBytes())
            assertFalse(File(file.path + EncryptedDatabaseMigrator.STAGING_SUFFIX).exists())
            assertFalse(File(directory, DatabaseKeyStore.RECORD_NAME).exists())
        } finally {
            Os.chmod(directory.path, 448)
        }
        assertRecoveredConfig()
    }

    @Test fun unsupportedPlaintextVersionsArePreserved() {
        // Below the first schema and above the current one: neither can be
        // migrated, and neither may be modified while being refused.
        for (version in listOf(0, FOCUSBRIDGE_SCHEMA_VERSION + 1)) {
            val candidate = File(root, "unsupported-$version.db")
            plaintext(candidate, 3)
            SQLiteDatabase.openDatabase(candidate.path, null, SQLiteDatabase.OPEN_READWRITE).use { it.version = version }
            val before = candidate.readBytes()
            assertThrows(IllegalStateException::class.java) { EncryptedDatabaseMigrator(candidate).prepare(keys) }
            assertArrayEquals(before, candidate.readBytes())
            assertFalse(File(candidate.path + EncryptedDatabaseMigrator.STAGING_SUFFIX).exists())
        }
    }

    @Test fun failedRoomSchemaMigrationRollsBackWithoutDestructiveFallback() {
        plaintext(file, 2)
        SQLiteDatabase.openDatabase(file.path, null, SQLiteDatabase.OPEN_READWRITE).use {
            it.execSQL("CREATE TABLE app_rules (unexpected TEXT)")
        }
        val room = DatabaseModule.buildEncryptedDatabase(context, file.name)
        try {
            assertThrows(IllegalStateException::class.java) { room.openHelper.writableDatabase }
        } finally {
            room.close()
        }
        val passphrase = EncryptedDatabaseMigrator(file).prepare(keys)
        try {
            encrypted(file, passphrase) {
                assertEquals(2, it.version)
                assertEquals("setting-value", scalar(it, "SELECT value FROM config"))
                assertEquals("unexpected", scalar(it, "SELECT name FROM pragma_table_info('app_rules')"))
            }
        } finally {
            passphrase.fill(0)
        }
    }

    private fun assertRecoveredConfig() {
        val passphrase = EncryptedDatabaseMigrator(file).prepare(keys)
        try {
            encrypted(file, passphrase) { assertEquals("setting-value", scalar(it, "SELECT value FROM config")) }
            assertFalse(EncryptedDatabaseMigrator.hasPlaintextHeader(file))
        } finally {
            passphrase.fill(0)
        }
    }

    private fun plaintext(target: File, version: Int) {
        SQLiteDatabase.openOrCreateDatabase(target, null).use { db ->
            db.execSQL("CREATE TABLE notifications (id TEXT NOT NULL PRIMARY KEY, appName TEXT NOT NULL, packageName TEXT NOT NULL, sender TEXT, message TEXT, timestamp INTEGER NOT NULL, receivedAt INTEGER NOT NULL, status TEXT NOT NULL, priority TEXT NOT NULL, contentHidden INTEGER NOT NULL, batchId TEXT)")
            val candidates = if (version > 1) ", endpointCandidates TEXT NOT NULL" else ""
            db.execSQL("CREATE TABLE pairings (deviceId TEXT NOT NULL PRIMARY KEY, endpoint TEXT NOT NULL, pairingKey TEXT NOT NULL, certFingerprint TEXT NOT NULL, mode TEXT NOT NULL, createdAt INTEGER NOT NULL, active INTEGER NOT NULL$candidates)")
            db.execSQL("CREATE TABLE config (`key` TEXT NOT NULL PRIMARY KEY, value TEXT NOT NULL)")
            if (version == 3) {
                db.execSQL("CREATE TABLE app_rules (packageName TEXT NOT NULL PRIMARY KEY, muted INTEGER NOT NULL, priority INTEGER NOT NULL, studySafe INTEGER NOT NULL, updatedAt INTEGER NOT NULL)")
                db.execSQL("INSERT INTO app_rules VALUES ('package', 0, 1, 0, 123)")
            }
            db.execSQL("INSERT INTO notifications VALUES ('notification-1', 'App', 'package', NULL, 'private notification', 123, 124, 'PENDING', 'NORMAL', 0, NULL)")
            db.execSQL("INSERT INTO pairings VALUES ('device', 'endpoint', 'pairing-secret', 'fingerprint', 'LOCAL', 123, 1${if (version > 1) ", 'candidate'" else ""})")
            db.execSQL("INSERT INTO config VALUES ('setting', 'setting-value')")
            db.execSQL("CREATE TABLE audit (id INTEGER PRIMARY KEY AUTOINCREMENT, payload BLOB)")
            db.execSQL("INSERT INTO audit VALUES (45, X'000102FF')")
            db.execSQL("CREATE INDEX audit_payload ON audit(payload)")
            db.execSQL("CREATE VIEW notification_messages AS SELECT message FROM notifications")
            db.execSQL("CREATE TRIGGER audit_changes AFTER UPDATE ON config BEGIN INSERT INTO audit(payload) VALUES (NEW.value); END")
            db.version = version
        }
    }

    private fun encrypted(target: File, key: ByteArray, block: (net.zetetic.database.sqlcipher.SQLiteDatabase) -> Unit) {
        net.zetetic.database.sqlcipher.SQLiteDatabase.openDatabase(
            target.path, key, null,
            net.zetetic.database.sqlcipher.SQLiteDatabase.OPEN_READONLY,
            EncryptedDatabaseMigrator.PRESERVE_ON_CORRUPTION, null,
        ).use(block)
    }

    private fun scalar(db: net.zetetic.database.sqlcipher.SQLiteDatabase, sql: String): String =
        db.rawQuery(sql, null).use { check(it.moveToFirst()); it.getString(0) }
}
