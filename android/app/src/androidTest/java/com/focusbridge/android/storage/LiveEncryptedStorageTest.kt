package com.focusbridge.android.storage

import android.database.sqlite.SQLiteDatabase
import android.database.sqlite.SQLiteException
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.focusbridge.android.data.local.FocusBridgeDatabase
import com.focusbridge.android.di.DatabaseModule
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

/**
 * Covers the path the shipping app actually takes: preparation started at process
 * start, then the Room provider consuming that result.
 *
 * The migration fixtures exercise [EncryptedDatabaseMigrator] directly. This adds
 * the wiring around it, so a provider that quietly opened a plaintext database
 * would fail here even while every fixture still passed.
 */
@RunWith(AndroidJUnit4::class)
class LiveEncryptedStorageTest {
    private val context = ApplicationProvider.getApplicationContext<android.content.Context>()
    private val name = "live-storage-proof.db"
    private lateinit var file: File
    private var database: FocusBridgeDatabase? = null

    @Before
    fun setUp() {
        file = context.getDatabasePath(name)
        cleanUp()
        DatabasePreparation.resetForTest()
    }

    @After
    fun tearDown() {
        database?.close()
        database = null
        DatabasePreparation.resetForTest()
        cleanUp()
    }

    private fun cleanUp() {
        for (suffix in listOf("", "-wal", "-shm", "-journal", ".encrypting", ".encryption.lock")) {
            File(file.path + suffix).delete()
        }
        File(context.noBackupFilesDir, "focusbridge-database-key.v1").delete()
    }

    private fun open(): FocusBridgeDatabase {
        val passphrase = DatabasePreparation.take(context, name)
        return DatabaseModule.buildFrom(context, name, passphrase).also { database = it }
    }

    @Test
    fun theProviderPathStoresNotificationsUnreadableByOrdinarySqlite() {
        open().openHelper.writableDatabase.execSQL(
            "INSERT INTO notifications (id, appName, packageName, sender, message, timestamp, " +
                "receivedAt, priority, status, contentHidden) VALUES " +
                "('live-1','App','com.example','Someone','TOP SECRET MESSAGE',1,1,'normal','PENDING',0)",
        )
        database!!.close()
        database = null

        assertTrue(file.exists())
        // No plaintext SQLite header, and the secret must not appear on disk.
        val bytes = file.readBytes()
        assertFalse(bytes.decodeToString(0, minOf(16, bytes.size)).startsWith("SQLite format 3"))
        assertFalse(
            "notification content must not be readable on disk",
            bytes.toString(Charsets.ISO_8859_1).contains("TOP SECRET MESSAGE"),
        )
        // An implementation with no knowledge of the key cannot read a page.
        // The error handler is essential: Android's default one deletes a
        // database it decides is corrupt, which would destroy the evidence and
        // the user's data along with it.
        assertThrows(SQLiteException::class.java) {
            SQLiteDatabase.openDatabase(
                file.path,
                null,
                SQLiteDatabase.OPEN_READONLY,
                android.database.DatabaseErrorHandler { throw SQLiteException("Preserve the database") },
            ).use { db -> db.rawQuery("SELECT * FROM notifications", null).use { it.moveToFirst() } }
        }
        // The rejected read must not have altered the file.
        assertArrayEquals(bytes, file.readBytes())
    }

    @Test
    fun preparedStorageReopensAndKeepsItsRows() {
        open().openHelper.writableDatabase.execSQL(
            "INSERT INTO config (key, value) VALUES ('live-key', 'live-value')",
        )
        database!!.close()
        database = null

        DatabasePreparation.resetForTest()
        val reopened = open().openHelper.readableDatabase
        reopened.query("SELECT value FROM config WHERE key='live-key'").use {
            assertTrue(it.moveToFirst())
            assertEquals("live-value", it.getString(0))
        }
    }

    @Test
    fun theSamePassphraseIsNeverHandedOutTwice() {
        open()
        // A second consumer would be a second, racing Room instance over one file.
        assertThrows(IllegalStateException::class.java) {
            DatabasePreparation.take(context, name)
        }
    }
}
