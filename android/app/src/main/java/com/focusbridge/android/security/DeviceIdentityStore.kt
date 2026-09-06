package com.focusbridge.android.security

import android.content.Context
import java.io.File
import java.io.IOException
import java.security.GeneralSecurityException
import java.security.MessageDigest
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Holds this phone's long-lived Curve25519 static private key for the device-only
 * Noise sessions, wrapped by a non-exportable AndroidKeyStore AES key.
 *
 * The key is written once and never replaced: the desktop pins the matching public
 * key at enrollment, so silently rotating it would present the phone as an unknown
 * device rather than recovering the pairing. It is deliberately kept separate from
 * the database passphrase (distinct alias, record, and additional authenticated
 * data) so neither wrapped secret can be substituted for the other.
 *
 * Reuses [DatabaseKeyStore.DiskStorage] and [DatabaseKeyStore.AndroidWrappingKeys],
 * which already implement durable write-once storage and keystore key generation.
 */
internal class DeviceIdentityStore(
    private val storage: DatabaseKeyStore.WrappedKeyStorage,
    private val wrappingKeys: DatabaseKeyStore.WrappingKeys,
) {
    constructor(context: Context) : this(
        DatabaseKeyStore.DiskStorage(File(context.noBackupFilesDir, RECORD_NAME)),
        DatabaseKeyStore.AndroidWrappingKeys("${context.packageName}.identity.wrapping.v1"),
    )

    /**
     * Returns a fresh copy of the private key, creating it on first use. The caller
     * owns the returned array and must wipe it; the native session consumes it.
     */
    fun getOrCreate(): ByteArray {
        val record = storage.read()
        if (record != null) {
            val wrappingKey = wrappingKeys.find()
                ?: throw GeneralSecurityException("Device identity wrapping key is missing; re-pair this phone")
            return unwrap(record, wrappingKey)
        }

        val wrappingKey = wrappingKeys.find() ?: wrappingKeys.generate()
        val identity = ByteArray(KEY_BYTES).also { SecureRandom().nextBytes(it) }
        try {
            val cipher = Cipher.getInstance(TRANSFORMATION).apply {
                init(Cipher.ENCRYPT_MODE, wrappingKey)
                updateAAD(AAD)
            }
            check(cipher.iv.size == IV_BYTES) { "Unexpected wrapping IV length" }
            storage.writeOnce(MAGIC + cipher.iv + cipher.doFinal(identity))
            val persisted = storage.read() ?: throw IOException("Device identity was not persisted")
            // Prove the record round-trips before any desktop pins the matching
            // public key; an unreadable identity must fail now, not at first sync.
            val recovered = unwrap(persisted, wrappingKey)
            try {
                check(MessageDigest.isEqual(identity, recovered)) { "Device identity verification failed" }
            } finally {
                recovered.fill(0)
            }
            return identity
        } catch (failure: Throwable) {
            identity.fill(0)
            throw failure
        }
    }

    private fun unwrap(record: ByteArray, wrappingKey: SecretKey): ByteArray {
        if (record.size != RECORD_BYTES || !record.copyOfRange(0, MAGIC.size).contentEquals(MAGIC)) {
            throw GeneralSecurityException("Invalid device identity record; re-pair this phone")
        }
        return Cipher.getInstance(TRANSFORMATION).run {
            init(Cipher.DECRYPT_MODE, wrappingKey, GCMParameterSpec(128, record, MAGIC.size, IV_BYTES))
            updateAAD(AAD)
            doFinal(record, MAGIC.size + IV_BYTES, KEY_BYTES + TAG_BYTES)
        }
    }

    companion object {
        internal const val RECORD_NAME = "focusbridge-device-identity.v1"
        private const val KEY_BYTES = 32
        private const val IV_BYTES = 12
        private const val TAG_BYTES = 16
        private const val RECORD_BYTES = 4 + IV_BYTES + KEY_BYTES + TAG_BYTES
        private const val TRANSFORMATION = "AES/GCM/NoPadding"
        private val MAGIC = byteArrayOf(0x46, 0x44, 0x49, 0x01)
        private val AAD = "FocusBridge/device-identity/v1".toByteArray(Charsets.US_ASCII) + MAGIC
    }
}
