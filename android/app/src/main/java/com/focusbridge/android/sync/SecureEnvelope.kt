package com.focusbridge.android.sync

import android.util.Base64
import java.nio.charset.StandardCharsets
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.Mac
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

object SecureEnvelope {
    private const val INFO = "FocusBridge message encryption v1"
    private val random = SecureRandom()

    fun encrypt(pairingKey: String, plaintextEnvelope: String): String {
        val nonce = ByteArray(12).also(random::nextBytes)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, SecretKeySpec(deriveKey(pairingKey), "AES"), GCMParameterSpec(128, nonce))
        val ciphertext = cipher.doFinal(plaintextEnvelope.toByteArray(StandardCharsets.UTF_8))
        return Protocol.json.encodeToString(
            Envelope.serializer(),
            Envelope(
                type = MessageType.ENCRYPTED,
                payload = buildJsonObject {
                    put("alg", "AES-256-GCM")
                    put("nonce", Base64.encodeToString(nonce, Base64.NO_WRAP))
                    put("ciphertext", Base64.encodeToString(ciphertext, Base64.NO_WRAP))
                },
            ),
        )
    }

    fun decrypt(pairingKey: String, payload: JsonObject): String {
        val nonce = Base64.decode(payload["nonce"]!!.toString().trim('"'), Base64.NO_WRAP)
        val ciphertext = Base64.decode(payload["ciphertext"]!!.toString().trim('"'), Base64.NO_WRAP)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, SecretKeySpec(deriveKey(pairingKey), "AES"), GCMParameterSpec(128, nonce))
        return String(cipher.doFinal(ciphertext), StandardCharsets.UTF_8)
    }

    private fun deriveKey(pairingKey: String): ByteArray {
        val prk = hmac("focusbridge-v1".toByteArray(StandardCharsets.UTF_8), pairingKey.toByteArray(StandardCharsets.UTF_8))
        val okm = hmac(prk, INFO.toByteArray(StandardCharsets.UTF_8) + byteArrayOf(1))
        return okm.copyOf(32)
    }

    private fun hmac(key: ByteArray, data: ByteArray): ByteArray {
        val mac = Mac.getInstance("HmacSHA256")
        mac.init(SecretKeySpec(key, "HmacSHA256"))
        return mac.doFinal(data)
    }
}
