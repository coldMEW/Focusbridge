package com.focusbridge.android.security

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.system.Os
import android.system.OsConstants
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.IOException
import java.security.GeneralSecurityException
import java.security.KeyStore
import java.security.MessageDigest
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** Call under the database initialization lock. Never recover key loss by replacing a key. */
internal class DatabaseKeyStore(
    private val storage: WrappedKeyStorage,
    private val wrappingKeys: WrappingKeys,
) {
    constructor(context: Context) : this(
        DiskStorage(File(context.noBackupFilesDir, RECORD_NAME)),
        AndroidWrappingKeys(alias(context)),
    )

    fun getOrCreate(encryptedDataExists: Boolean): ByteArray {
        val record = storage.read()
        if (record != null) {
            validateRecord(record)
            val key = wrappingKeys.find()
                ?: throw GeneralSecurityException("Database wrapping key is missing; data was not modified")
            return unwrap(record, key)
        }
        if (encryptedDataExists) {
            throw GeneralSecurityException("Database key record is missing; data was not modified")
        }

        val wrappingKey = wrappingKeys.find() ?: wrappingKeys.generate()
        val passphrase = ByteArray(KEY_BYTES).also { SecureRandom().nextBytes(it) }
        try {
            val cipher = Cipher.getInstance(TRANSFORMATION).apply {
                init(Cipher.ENCRYPT_MODE, wrappingKey)
                updateAAD(AAD)
            }
            check(cipher.iv.size == IV_BYTES)
            storage.writeOnce(MAGIC + cipher.iv + cipher.doFinal(passphrase))
            val persisted = storage.read()
                ?: throw IOException("Database key record was not persisted")
            val recovered = unwrap(persisted, wrappingKey)
            try {
                check(MessageDigest.isEqual(passphrase, recovered)) { "Database key verification failed" }
            } finally {
                recovered.fill(0)
            }
            return passphrase
        } catch (failure: Throwable) {
            passphrase.fill(0)
            throw failure
        }
    }

    private fun validateRecord(record: ByteArray) {
        if (record.size != RECORD_BYTES || !record.copyOfRange(0, MAGIC.size).contentEquals(MAGIC)) {
            throw GeneralSecurityException("Invalid database key record; data was not modified")
        }
    }

    private fun unwrap(record: ByteArray, wrappingKey: SecretKey): ByteArray {
        validateRecord(record)
        return Cipher.getInstance(TRANSFORMATION).run {
            init(Cipher.DECRYPT_MODE, wrappingKey, GCMParameterSpec(128, record, MAGIC.size, IV_BYTES))
            updateAAD(AAD)
            doFinal(record, MAGIC.size + IV_BYTES, KEY_BYTES + TAG_BYTES)
        }
    }

    interface WrappedKeyStorage {
        fun read(): ByteArray?
        fun writeOnce(record: ByteArray)
    }

    interface WrappingKeys {
        fun find(): SecretKey?
        fun generate(): SecretKey
    }

    internal class AndroidWrappingKeys(private val alias: String) : WrappingKeys {
        private fun keyStore(): KeyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }

        override fun find(): SecretKey? {
            val store = keyStore()
            if (!store.containsAlias(alias)) return null
            return store.getKey(alias, null) as? SecretKey
                ?: throw GeneralSecurityException("Invalid database wrapping key")
        }

        override fun generate(): SecretKey {
            check(!keyStore().containsAlias(alias)) { "Refusing to replace database wrapping key" }
            return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").run {
                init(
                    KeyGenParameterSpec.Builder(
                        alias,
                        KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
                    )
                        .setKeySize(256)
                        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                        .setRandomizedEncryptionRequired(true)
                        // Notification sync must work without a biometric or per-use unlock prompt.
                        .setUserAuthenticationRequired(false)
                        .build(),
                )
                generateKey()
            }
        }
    }

    internal class DiskStorage(private val file: File) : WrappedKeyStorage {
        private val pending = File(file.path + ".pending")

        override fun read(): ByteArray? {
            if (pending.exists()) {
                throw IOException("Interrupted database key write; preserve key files for recovery")
            }
            if (!file.exists()) return null
            if (!file.isFile || file.length() != RECORD_BYTES.toLong()) {
                throw GeneralSecurityException("Invalid database key record")
            }
            return file.readBytes()
        }

        override fun writeOnce(record: ByteArray) {
            check(!file.exists() && !pending.exists()) { "Refusing to replace database key record" }
            val directory = requireNotNull(file.parentFile)
            check(directory.isDirectory || directory.mkdirs()) { "Cannot create database key directory" }
            check(pending.createNewFile()) { "Database key write already in progress" }
            FileOutputStream(pending).use {
                it.write(record)
                it.fd.sync()
            }
            Os.rename(pending.path, file.path)
            DurableDatabaseFiles.syncDirectory(directory)
            // Also persist a newly created no-backup directory before encrypting any user data.
            directory.parentFile?.let { DurableDatabaseFiles.syncDirectory(it) }
        }
    }

    companion object {
        internal const val RECORD_NAME = "focusbridge-database-key.v1"
        internal fun alias(context: Context): String = "${context.packageName}.database.wrapping.v1"
        private const val KEY_BYTES = 32
        private const val IV_BYTES = 12
        private const val TAG_BYTES = 16
        private const val RECORD_BYTES = 4 + IV_BYTES + KEY_BYTES + TAG_BYTES
        private const val TRANSFORMATION = "AES/GCM/NoPadding"
        private val MAGIC = byteArrayOf(0x46, 0x44, 0x42, 0x01)
        private val AAD = "FocusBridge/database-passphrase/v1".toByteArray(Charsets.US_ASCII) + MAGIC
    }
}

internal object DurableDatabaseFiles {
    fun syncFile(file: File) = FileInputStream(file).use { it.fd.sync() }

    fun syncDirectory(directory: File) {
        check(directory.isDirectory) { "Database parent is not a directory" }
        val descriptor = Os.open(directory.path, OsConstants.O_RDONLY, 0)
        try {
            Os.fsync(descriptor)
        } finally {
            Os.close(descriptor)
        }
    }
}
