package com.focusbridge.android.storage

import android.content.Context
import android.content.ContextWrapper
import android.database.sqlite.SQLiteDatabase
import android.os.Build
import android.os.Process
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.focusbridge.android.security.DatabaseKeyStore
import com.focusbridge.android.security.DurableDatabaseFiles
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.security.KeyStore
import org.junit.Assert.*
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith

/** Host-driven two-invocation proof. SIGKILL cannot run catch/finally or JUnit teardown. */
@RunWith(AndroidJUnit4::class)
class StorageProcessDeathTest {
    @Test fun survivesRealProcessDeath() {
        val arguments = InstrumentationRegistry.getArguments()
        val action = arguments.getString("storageProofAction")
        assumeTrue("Run with run-storage-proof.ps1", action != null)
        check(action == "kill" || action == "recover")
        check(Build.FINGERPRINT.contains("generic") || Build.MODEL.contains("sdk", ignoreCase = true))
        val id = requireNotNull(arguments.getString("storageProofId"))
        check(id.matches(Regex("[a-zA-Z0-9-]{1,80}")))
        val phase = requireNotNull(arguments.getString("storageProofPhase"))
        check(phase in EncryptedDatabaseMigrator.Phase.values().map { it.name } + listOf("WAL_COMMITTED", "KEY_PENDING"))
        val base = InstrumentationRegistry.getInstrumentation().context
        val cache = InstrumentationRegistry.getInstrumentation().targetContext.cacheDir
        val root = File(cache, "storage-process-death-$id")
        val context = object : ContextWrapper(base) {
            override fun getApplicationContext(): Context = this
            override fun getPackageName(): String = "${base.packageName}.death.$id"
            override fun getDatabasePath(name: String): File = File(root, name)
            override fun getNoBackupFilesDir(): File = File(root, "keys").apply { mkdirs() }
        }
        val file = context.getDatabasePath("fixture.db")
        val marker = File(root, "killed-at")
        val keys = DatabaseKeyStore(context)
        System.loadLibrary("sqlcipher")

        fun killNow() : Nothing {
            FileOutputStream(marker).use {
                it.write("$phase:${Process.myPid()}".toByteArray(Charsets.US_ASCII))
                it.fd.sync()
            }
            DurableDatabaseFiles.syncDirectory(root)
            Process.killProcess(Process.myPid())
            Thread.sleep(5_000)
            error("SIGKILL did not terminate the process")
        }

        if (action == "kill") {
            check(!root.exists() && root.mkdirs()) { "Refusing to overwrite an earlier proof" }
            SQLiteDatabase.openOrCreateDatabase(file, null).use { db ->
                db.execSQL("CREATE TABLE config (`key` TEXT NOT NULL PRIMARY KEY, value TEXT NOT NULL)")
                db.execSQL("INSERT INTO config VALUES ('setting', 'survives-sigkill')")
                db.version = 3
                if (phase == "WAL_COMMITTED") {
                    check(db.enableWriteAheadLogging())
                    db.rawQuery("PRAGMA wal_autocheckpoint=0", null).use { check(it.moveToFirst()) }
                    db.execSQL("INSERT INTO config VALUES ('wal-only', 'committed-before-sigkill')")
                    check(File(file.path + "-wal").length() > 0)
                    killNow()
                }
            }
            if (phase == "KEY_PENDING") {
                val record = File(context.noBackupFilesDir, DatabaseKeyStore.RECORD_NAME)
                val disk = DatabaseKeyStore.DiskStorage(record)
                DatabaseKeyStore(object : DatabaseKeyStore.WrappedKeyStorage {
                    override fun read(): ByteArray? = disk.read()
                    override fun writeOnce(record: ByteArray) {
                        // A real partially written key file, not an exception masquerading as death.
                        FileOutputStream(File(context.noBackupFilesDir, DatabaseKeyStore.RECORD_NAME + ".pending")).use {
                            it.write(record, 0, 17)
                            it.fd.sync()
                        }
                        killNow()
                    }
                }, DatabaseKeyStore.AndroidWrappingKeys(DatabaseKeyStore.alias(context))).getOrCreate(false)
            } else {
                EncryptedDatabaseMigrator(file) { if (it.name == phase) killNow() }.prepare(keys).fill(0)
            }
            error("Requested death boundary was never reached")
        }

        check(root.isDirectory && marker.isFile) { "No completed death marker; this is not recovery evidence" }
        val killed = marker.readText().split(':')
        assertEquals(phase, killed[0])
        assertNotEquals(killed[1].toInt(), Process.myPid())
        assertTrue(file.exists())
        if (phase == "KEY_PENDING") {
            val record = File(context.noBackupFilesDir, DatabaseKeyStore.RECORD_NAME)
            val pending = File(record.path + ".pending")
            val before = pending.readBytes()
            val source = file.readBytes()
            assertEquals(17, before.size)
            assertThrows(IOException::class.java) { EncryptedDatabaseMigrator(file).prepare(keys) }
            assertArrayEquals(before, pending.readBytes())
            assertArrayEquals(source, file.readBytes())
            assertFalse(record.exists())
            assertNotNull(DatabaseKeyStore.AndroidWrappingKeys(DatabaseKeyStore.alias(context)).find())
        } else {
            assertEquals(phase !in listOf("REPLACED", "COMMITTED"), EncryptedDatabaseMigrator.hasPlaintextHeader(file))
            val passphrase = EncryptedDatabaseMigrator(file).prepare(keys)
            try {
                net.zetetic.database.sqlcipher.SQLiteDatabase.openDatabase(
                    file.path, passphrase, null, net.zetetic.database.sqlcipher.SQLiteDatabase.OPEN_READONLY,
                    EncryptedDatabaseMigrator.PRESERVE_ON_CORRUPTION, null,
                ).use { db ->
                    db.rawQuery("SELECT value FROM config WHERE key='setting'", null).use {
                        assertTrue(it.moveToFirst())
                        assertEquals("survives-sigkill", it.getString(0))
                    }
                    if (phase == "WAL_COMMITTED") {
                        db.rawQuery("SELECT value FROM config WHERE key='wal-only'", null).use {
                            assertTrue(it.moveToFirst())
                            assertEquals("committed-before-sigkill", it.getString(0))
                        }
                    }
                }
                assertFalse(EncryptedDatabaseMigrator.hasPlaintextHeader(file))
                assertFalse(File(file.path + EncryptedDatabaseMigrator.STAGING_SUFFIX).exists())
            } finally {
                passphrase.fill(0)
            }
        }
        // Clean up only this UUID-scoped synthetic fixture, and only after all recovery assertions pass.
        KeyStore.getInstance("AndroidKeyStore").apply { load(null) }.deleteEntry(DatabaseKeyStore.alias(context))
        check(root.deleteRecursively())
    }
}
