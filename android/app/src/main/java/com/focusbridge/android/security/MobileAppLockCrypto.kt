package com.focusbridge.android.security

import java.security.MessageDigest
import java.security.SecureRandom
import java.util.Base64
import javax.crypto.SecretKeyFactory
import javax.crypto.spec.PBEKeySpec

object MobileAppLockCrypto {
    fun newSalt(): String {
        val bytes = ByteArray(16)
        SecureRandom().nextBytes(bytes)
        return Base64.getEncoder().encodeToString(bytes)
    }

    fun hashSecret(secret: String, salt: String): String {
        val spec = PBEKeySpec(
            secret.trim().toCharArray(),
            Base64.getDecoder().decode(salt),
            120_000,
            256,
        )
        val hash = SecretKeyFactory.getInstance("PBKDF2WithHmacSHA256").generateSecret(spec).encoded
        return Base64.getEncoder().encodeToString(hash)
    }

    fun verify(secret: String, salt: String?, expectedHash: String?): Boolean {
        if (salt.isNullOrBlank() || expectedHash.isNullOrBlank() || secret.isBlank()) return false
        val actual = hashSecret(secret, salt)
        return MessageDigest.isEqual(actual.toByteArray(), expectedHash.toByteArray())
    }

    fun validSecret(secret: String): Boolean {
        val trimmed = secret.trim()
        val isPin = trimmed.length >= 4 && trimmed.all(Char::isDigit)
        return isPin || trimmed.length >= 8
    }
}
