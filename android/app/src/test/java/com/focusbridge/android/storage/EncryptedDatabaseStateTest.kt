package com.focusbridge.android.storage

import java.io.File
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class EncryptedDatabaseStateTest {
    @get:Rule val temporary = TemporaryFolder()

    @Test fun freshInstallationCanCreateKey() {
        assertFalse(EncryptedDatabaseMigrator.requiresExistingKey(File(temporary.root, "new.db")))
    }

    @Test fun plaintextWithWalCanCreateKey() {
        val file = plaintext()
        File(file.path + "-wal").writeBytes(ByteArray(32))
        assertFalse(EncryptedDatabaseMigrator.requiresExistingKey(file))
    }

    @Test fun truncatedOrUnknownDatabaseRequiresExistingKey() {
        val file = File(temporary.root, "unknown.db")
        for (bytes in listOf(ByteArray(0), ByteArray(32) { 5 }, "SQLite format 3".toByteArray())) {
            file.writeBytes(bytes)
            assertTrue(EncryptedDatabaseMigrator.requiresExistingKey(file))
        }
    }

    @Test fun everyOrphanSidecarRequiresExistingKey() {
        for (suffix in listOf("-wal", "-shm", "-journal")) {
            val file = File(temporary.root, "orphan$suffix.db")
            File(file.path + suffix).writeBytes(ByteArray(32))
            assertTrue(EncryptedDatabaseMigrator.requiresExistingKey(file))
        }
    }

    @Test fun everyStagingArtifactRequiresExistingKeyEvenWithPlaintextSource() {
        for (suffix in listOf("", "-wal", "-shm", "-journal")) {
            val file = plaintext("stage$suffix.db")
            File(file.path + EncryptedDatabaseMigrator.STAGING_SUFFIX + suffix).writeBytes(ByteArray(32))
            assertTrue(EncryptedDatabaseMigrator.requiresExistingKey(file))
        }
    }

    @Test fun stagingWordInDirectoryNameDoesNotTurnPlaintextWalIntoEncryptedEvidence() {
        val directory = temporary.newFolder("test.encrypted-staging")
        val file = File(directory, "plain.db")
        file.writeBytes("SQLite format 3\u0000".toByteArray())
        File(file.path + "-wal").writeBytes(ByteArray(32))
        assertFalse(EncryptedDatabaseMigrator.requiresExistingKey(file))
    }

    private fun plaintext(name: String = "plain.db"): File = File(temporary.root, name).apply {
        writeBytes("SQLite format 3\u0000".toByteArray())
    }
}
