package com.focusbridge.android.storage

import com.focusbridge.android.data.local.FOCUSBRIDGE_SCHEMA_VERSION
import android.database.Cursor
import android.database.sqlite.SQLiteException
import android.system.Os
import androidx.sqlite.db.SupportSQLiteOpenHelper
import com.focusbridge.android.security.DatabaseKeyStore
import com.focusbridge.android.security.DurableDatabaseFiles
import java.io.DataOutputStream
import java.io.File
import java.io.IOException
import java.io.OutputStream
import java.io.RandomAccessFile
import java.security.DigestOutputStream
import java.security.MessageDigest
import net.zetetic.database.DatabaseErrorHandler
import net.zetetic.database.sqlcipher.SQLiteDatabase
import net.zetetic.database.sqlcipher.SQLiteOpenHelper

/**
 * Offline startup operation: no Room/SQLite handle or other writer may be open on this database.
 * The original remains authoritative until a verified, fsynced export atomically replaces it.
 * Restart before rename re-exports; restart after rename validates the encrypted original path.
 * No plaintext backup is created. Unlinking is not a promise of physical flash erasure.
 */
internal class EncryptedDatabaseMigrator(
    private val database: File,
    private val reached: (Phase) -> Unit = {},
) {
    private val directory = requireNotNull(database.parentFile)
    private val staging = File(database.path + STAGING_SUFFIX)

    fun prepare(keys: DatabaseKeyStore): ByteArray = synchronized(INITIALIZATION_LOCK) {
        check(directory.isDirectory || directory.mkdirs()) { "Cannot create database directory" }
        RandomAccessFile(File(directory, "${database.name}.encryption.lock"), "rw").use { lockFile ->
            lockFile.channel.lock().use {
                System.loadLibrary("sqlcipher")
                val passphrase = keys.getOrCreate(requiresExistingKey(database))
                try {
                    when {
                        !database.exists() -> check(!artifacts(database).any { it.exists() }) {
                            "Database source is missing; preserve artifacts for recovery"
                        }
                        hasPlaintextHeader(database) -> migrate(passphrase)
                        else -> {
                            check(!stagingArtifacts().any { it.exists() }) {
                                "Unexpected staging data beside encrypted database; preserve for recovery"
                            }
                            open(database, passphrase, SQLiteDatabase.OPEN_READONLY).use { db ->
                                verifyEncrypted(db)
                                check(db.version in 1..FOCUSBRIDGE_SCHEMA_VERSION) { "Unsupported database version; data preserved" }
                            }
                        }
                    }
                    passphrase
                } catch (failure: Throwable) {
                    passphrase.fill(0)
                    throw failure
                }
            }
        }
    }

    private fun migrate(passphrase: ByteArray) {
        val expected = open(database, ByteArray(0), SQLiteDatabase.OPEN_READWRITE).use { source ->
            verifyIntegrity(source)
            check(source.version in 1..FOCUSBRIDGE_SCHEMA_VERSION) { "Unsupported plaintext database version; data preserved" }
            // Never unlink WAL manually: committed rows may exist only there.
            source.rawQuery("PRAGMA wal_checkpoint(TRUNCATE)", null).use {
                check(it.moveToFirst() && it.getInt(0) == 0 && it.getInt(1) == it.getInt(2)) {
                    "Database WAL checkpoint is busy or incomplete; data preserved"
                }
            }
            source.rawQuery("PRAGMA journal_mode=DELETE", null).use {
                check(it.moveToFirst() && it.getString(0).equals("delete", ignoreCase = true)) {
                    "Database is still in use; cannot switch journal mode"
                }
            }
            snapshot(source)
        }
        retireColdJournal(database)
        requireNoSidecars(database)
        DurableDatabaseFiles.syncFile(database)
        DurableDatabaseFiles.syncDirectory(directory)
        reached(Phase.CHECKPOINTED)

        // Only the known staging files are disposable, and only after validating the source.
        for (artifact in stagingArtifacts()) {
            if (artifact.exists() && !artifact.delete()) throw IOException("Cannot remove incomplete encrypted export")
        }
        DurableDatabaseFiles.syncDirectory(directory)
        open(staging, passphrase, SQLiteDatabase.CREATE_IF_NECESSARY).use { target ->
            target.execSQL("PRAGMA synchronous=FULL")
            target.execSQL("PRAGMA temp_store=MEMORY")
            target.execSQL("ATTACH DATABASE ? AS source KEY ''", arrayOf(database.path))
            target.execSQL("PRAGMA main.auto_vacuum=${expected.autoVacuum}")
            target.beginTransaction()
            try {
                check(snapshot(target, "source") == expected) { "Source changed before export; data preserved" }
                target.rawQuery("SELECT sqlcipher_export('main', 'source')", null).use { check(it.moveToFirst()) }
                // sqlcipher_export deliberately does not copy these header pragmas.
                target.version = expected.version
                target.execSQL("PRAGMA main.application_id=${expected.applicationId}")
                check(snapshot(target) == expected) { "Encrypted export did not preserve schema and contents" }
                target.setTransactionSuccessful()
            } finally {
                target.endTransaction()
            }
            target.execSQL("DETACH DATABASE source")
        }
        reached(Phase.EXPORTED)
        requireNoSidecars(staging)
        check(!hasPlaintextHeader(staging)) { "Export is not encrypted" }
        open(staging, passphrase, SQLiteDatabase.OPEN_READONLY).use { target ->
            verifyEncrypted(target)
            check(snapshot(target) == expected) { "Reopened encrypted export differs from source" }
        }
        reached(Phase.VERIFIED)
        open(database, ByteArray(0), SQLiteDatabase.OPEN_READONLY).use { source ->
            check(snapshot(source) == expected) { "Source changed after export; data preserved" }
        }
        requireNoSidecars(database)
        DurableDatabaseFiles.syncFile(staging)
        DurableDatabaseFiles.syncDirectory(directory)
        reached(Phase.BEFORE_REPLACE)
        // Same-directory POSIX rename replaces atomically; there is no missing-source rename gap.
        Os.rename(staging.path, database.path)
        reached(Phase.REPLACED)
        DurableDatabaseFiles.syncDirectory(directory)
        reached(Phase.COMMITTED)
    }

    private fun stagingArtifacts(): List<File> = listOf(staging) + sidecars(staging)

    /**
     * Removes a rollback journal that is provably cold, so the migrated database
     * is not left with a stale sidecar beside it.
     *
     * Only a zero-length journal qualifies. SQLite defines such a file as not
     * hot: it holds no pages to roll back, so unlinking it cannot lose a commit.
     * Anything with content is left untouched for [requireNoSidecars] to refuse.
     * Some Android versions leave this file behind even after the journal mode
     * transition above has retired it.
     */
    private fun retireColdJournal(file: File) {
        val journal = File(file.path + "-journal")
        if (journal.exists() && journal.length() == 0L && !journal.delete()) {
            throw IOException("Cannot remove cold rollback journal ${journal.name}")
        }
    }

    /**
     * Fails unless the database has no journal that could still hold committed
     * data. A write-ahead log or its shared index means rows may exist only
     * there, and a rollback journal with content means a transaction never
     * finished — pairing either with a migrated file would lose or corrupt data.
     *
     * A zero-length rollback journal is the one exception, and it is not a
     * loosening: SQLite defines a zero-length journal as not hot, so it carries
     * nothing to roll back. Android leaves one behind after a clean close in
     * DELETE mode, and refusing it would block every real migration.
     */
    private fun requireNoSidecars(file: File) {
        val present = sidecars(file).filter { it.exists() && (it.length() > 0 || !it.path.endsWith("-journal")) }
        check(present.isEmpty()) {
            // Name the artifacts: a lingering journal is the difference between a
            // safe abort and an operator guessing which file to preserve.
            "Database still has journal sidecars; refusing replacement: " +
                present.joinToString { "${it.name} (${it.length()} bytes)" }
        }
    }

    internal enum class Phase { CHECKPOINTED, EXPORTED, VERIFIED, BEFORE_REPLACE, REPLACED, COMMITTED }

    private data class Snapshot(
        val version: Int,
        val applicationId: Long,
        val autoVacuum: Long,
        val schema: List<List<String?>>,
        val tables: Map<String, List<Byte>>,
    )

    private fun snapshot(db: SQLiteDatabase, schemaName: String = "main"): Snapshot {
        val schema = mutableListOf<List<String?>>()
        val tables = linkedMapOf<String, List<Byte>>()
        db.rawQuery("SELECT type, name, tbl_name, sql FROM $schemaName.sqlite_master ORDER BY type, name", null).use { cursor ->
            while (cursor.moveToNext()) {
                schema += (0..3).map { if (cursor.isNull(it)) null else cursor.getString(it) }
            }
        }
        for (entry in schema.filter { it[0] == "table" }) {
            val tableName = requireNotNull(entry[1])
            val table = "$schemaName.${quoteIdentifier(tableName)}"
            val columns = db.rawQuery("SELECT * FROM $table LIMIT 0", null).use { it.columnNames }
            val order = columns.joinToString(",") { "${quoteIdentifier(it)} COLLATE BINARY" }
            val digest = MessageDigest.getInstance("SHA-256")
            val sink = DataOutputStream(DigestOutputStream(object : OutputStream() {
                override fun write(value: Int) = Unit
                override fun write(bytes: ByteArray, offset: Int, length: Int) = Unit
            }, digest))
            db.rawQuery("SELECT * FROM $table ORDER BY $order", null).use { rows ->
                var count = 0L
                while (rows.moveToNext()) {
                    count++
                    for (column in columns.indices) {
                        sink.writeInt(rows.getType(column))
                        when (rows.getType(column)) {
                            Cursor.FIELD_TYPE_NULL -> Unit
                            Cursor.FIELD_TYPE_INTEGER -> sink.writeLong(rows.getLong(column))
                            Cursor.FIELD_TYPE_FLOAT -> sink.writeLong(java.lang.Double.doubleToRawLongBits(rows.getDouble(column)))
                            Cursor.FIELD_TYPE_STRING, Cursor.FIELD_TYPE_BLOB -> {
                                val bytes = if (rows.getType(column) == Cursor.FIELD_TYPE_BLOB) rows.getBlob(column)
                                    else rows.getString(column).toByteArray(Charsets.UTF_8)
                                sink.writeInt(bytes.size)
                                sink.write(bytes)
                            }
                            else -> error("Unexpected SQLite value type")
                        }
                    }
                }
                sink.writeLong(count)
            }
            sink.close()
            tables[tableName] = digest.digest().toList()
        }
        return Snapshot(
            pragma(db, "$schemaName.user_version").toInt(),
            pragma(db, "$schemaName.application_id"),
            pragma(db, "$schemaName.auto_vacuum"),
            schema,
            tables,
        )
    }

    companion object {
        internal const val STAGING_SUFFIX = ".encrypted-staging"
        private val INITIALIZATION_LOCK = Any()
        private val PLAINTEXT_HEADER = "SQLite format 3\u0000".toByteArray(Charsets.US_ASCII)

        // Both SQLite and SQLCipher defaults may delete databases on corruption. Never delegate.
        internal val PRESERVE_ON_CORRUPTION = DatabaseErrorHandler { _, _ ->
            throw SQLiteException("Database corruption detected; all files preserved for recovery")
        }

        internal fun hasPlaintextHeader(file: File): Boolean {
            if (!file.exists()) return false
            return file.inputStream().use { stream ->
                val header = ByteArray(PLAINTEXT_HEADER.size)
                var read = 0
                while (read < header.size) {
                    val count = stream.read(header, read, header.size - read)
                    if (count < 0) return@use false
                    read += count
                }
                header.contentEquals(PLAINTEXT_HEADER)
            }
        }

        internal fun requiresExistingKey(database: File): Boolean {
            val staging = File(database.path + STAGING_SUFFIX)
            if ((listOf(staging) + sidecars(staging)).any { it.exists() }) return true
            if (!database.exists()) return sidecars(database).any { it.exists() }
            return !hasPlaintextHeader(database)
        }

        private fun sidecars(file: File): List<File> = listOf("-wal", "-shm", "-journal").map { File(file.path + it) }

        private fun artifacts(database: File): List<File> {
            val staging = File(database.path + STAGING_SUFFIX)
            return sidecars(database) + staging + sidecars(staging)
        }

        private fun open(file: File, key: ByteArray, flags: Int): SQLiteDatabase {
            val db = SQLiteDatabase.openDatabase(
                file.path, key, null, flags or SQLiteDatabase.NO_LOCALIZED_COLLATORS,
                PRESERVE_ON_CORRUPTION, null,
            )
            try {
                // Sorting plaintext source rows must not spill their values to on-disk temp files.
                db.execSQL("PRAGMA temp_store=MEMORY")
                return db
            } catch (failure: Throwable) {
                db.close()
                throw failure
            }
        }

        private fun verifyIntegrity(db: SQLiteDatabase) {
            db.rawQuery("PRAGMA integrity_check", null).use {
                check(it.moveToFirst() && it.getString(0) == "ok" && !it.moveToNext()) { "Database integrity check failed" }
            }
            db.rawQuery("PRAGMA foreign_key_check", null).use {
                check(!it.moveToFirst()) { "Database foreign-key check failed" }
            }
        }

        private fun verifyEncrypted(db: SQLiteDatabase) {
            db.rawQuery("SELECT count(*) FROM sqlite_master", null).use { check(it.moveToFirst()) }
            db.rawQuery("PRAGMA cipher_integrity_check", null).use {
                check(!it.moveToFirst()) { "Encrypted database authentication failed" }
            }
            verifyIntegrity(db)
        }

        private fun pragma(db: SQLiteDatabase, name: String): Long =
            db.rawQuery("PRAGMA $name", null).use { check(it.moveToFirst()); it.getLong(0) }

        private fun quoteIdentifier(name: String): String = "\"${name.replace("\"", "\"\"")}\""

        /** Like SupportOpenHelperFactory, but supplies a non-destructive corruption handler. */
        internal fun roomFactory(passphrase: ByteArray): SupportSQLiteOpenHelper.Factory =
            SupportSQLiteOpenHelper.Factory { configuration ->
                object : SQLiteOpenHelper(
                    configuration.context, configuration.name, passphrase, null,
                    configuration.callback.version, 0, PRESERVE_ON_CORRUPTION, null, false,
                ) {
                    override fun onConfigure(db: SQLiteDatabase) = configuration.callback.onConfigure(db)
                    override fun onCreate(db: SQLiteDatabase) = configuration.callback.onCreate(db)
                    override fun onUpgrade(db: SQLiteDatabase, oldVersion: Int, newVersion: Int) =
                        configuration.callback.onUpgrade(db, oldVersion, newVersion)
                    override fun onDowngrade(db: SQLiteDatabase, oldVersion: Int, newVersion: Int) =
                        configuration.callback.onDowngrade(db, oldVersion, newVersion)
                    override fun onOpen(db: SQLiteDatabase) = configuration.callback.onOpen(db)
                    override fun onBeforeDelete(db: SQLiteDatabase) {
                        throw SQLiteException("Refusing to delete database during upgrade")
                    }
                }
            }
    }
}
