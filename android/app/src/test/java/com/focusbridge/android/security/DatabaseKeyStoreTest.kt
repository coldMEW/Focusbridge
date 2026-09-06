package com.focusbridge.android.security

import java.io.IOException
import java.security.GeneralSecurityException
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertThrows
import org.junit.Test

class DatabaseKeyStoreTest {
    private val storage = MemoryStorage()
    private val keys = MemoryKeys()
    private val store = DatabaseKeyStore(storage, keys)

    @Test fun createsRandom256BitPassphraseAndReopensIt() {
        val first = store.getOrCreate(false)
        assertEquals(32, first.size)
        assertArrayEquals(first, DatabaseKeyStore(storage, keys).getOrCreate(true))
        assertEquals(1, keys.generations)
        assertEquals(1, storage.writes)
        assertFalse(first.contentEquals(storage.record))
        assertFalse(first.contentEquals(DatabaseKeyStore(MemoryStorage(), MemoryKeys()).getOrCreate(false)))
    }

    @Test fun missingWrappedKeyWithEncryptedDataDoesNotGenerateAnything() {
        assertThrows(GeneralSecurityException::class.java) { store.getOrCreate(true) }
        assertEquals(0, keys.generations)
        assertEquals(0, storage.writes)
    }

    @Test fun missingKeystoreAliasDoesNotReplaceAnExistingRecord() {
        store.getOrCreate(false)
        val record = storage.record!!.copyOf()
        keys.key = null
        assertThrows(GeneralSecurityException::class.java) { store.getOrCreate(true) }
        assertThrows(GeneralSecurityException::class.java) { store.getOrCreate(false) }
        assertEquals(1, keys.generations)
        assertArrayEquals(record, storage.record)
    }

    @Test fun corruptedCiphertextIsRejectedWithoutRegeneration() {
        store.getOrCreate(false)
        storage.record!![storage.record!!.lastIndex] = (storage.record!!.last().toInt() xor 1).toByte()
        assertThrows(GeneralSecurityException::class.java) { store.getOrCreate(true) }
        assertEquals(1, storage.writes)
        assertEquals(1, keys.generations)
    }

    @Test fun malformedRecordIsRejectedWithoutRegeneration() {
        for (record in listOf(byteArrayOf(), byteArrayOf(1), ByteArray(512))) {
            storage.record = record
            assertThrows(GeneralSecurityException::class.java) { store.getOrCreate(false) }
        }
        assertEquals(0, keys.generations)
        assertEquals(0, storage.writes)
    }

    @Test fun wrongWrappingKeyIsRejected() {
        store.getOrCreate(false)
        keys.key = newKey()
        assertThrows(GeneralSecurityException::class.java) { store.getOrCreate(true) }
        assertEquals(1, keys.generations)
    }

    @Test fun keystoreUnavailableDoesNotWriteOrFallBack() {
        keys.failure = GeneralSecurityException("Keystore unavailable")
        assertThrows(GeneralSecurityException::class.java) { store.getOrCreate(false) }
        assertEquals(0, storage.writes)
        assertEquals(0, keys.generations)
    }

    @Test fun persistenceFailureNeverReturnsAnUnrecoverablePassphrase() {
        storage.writeFailure = IOException("disk full")
        assertThrows(IOException::class.java) { store.getOrCreate(false) }
        assertNotNull(keys.key)
        assertEquals(1, keys.generations)
    }

    @Test fun readFailureIsNotTreatedAsFirstRun() {
        storage.readFailure = IOException("unreadable record")
        assertThrows(IOException::class.java) { store.getOrCreate(false) }
        assertEquals(0, keys.generations)
        assertEquals(0, storage.writes)
    }

    private class MemoryStorage : DatabaseKeyStore.WrappedKeyStorage {
        var record: ByteArray? = null
        var writes = 0
        var readFailure: IOException? = null
        var writeFailure: IOException? = null
        override fun read(): ByteArray? {
            readFailure?.let { throw it }
            return record?.copyOf()
        }
        override fun writeOnce(record: ByteArray) {
            writeFailure?.let { throw it }
            check(this.record == null)
            writes++
            this.record = record.copyOf()
        }
    }

    private class MemoryKeys : DatabaseKeyStore.WrappingKeys {
        var key: SecretKey? = null
        var generations = 0
        var failure: GeneralSecurityException? = null
        override fun find(): SecretKey? {
            failure?.let { throw it }
            return key
        }
        override fun generate(): SecretKey {
            failure?.let { throw it }
            check(key == null)
            generations++
            return newKey().also { key = it }
        }
    }

    companion object {
        private fun newKey(): SecretKey = KeyGenerator.getInstance("AES").apply { init(256) }.generateKey()
    }
}
