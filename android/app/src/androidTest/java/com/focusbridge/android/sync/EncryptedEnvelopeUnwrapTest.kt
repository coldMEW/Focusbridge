package com.focusbridge.android.sync

import androidx.test.ext.junit.runners.AndroidJUnit4
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The desktop sends AUTH_OK in the clear and wraps everything after it in the
 * pairing-key envelope. A transport that forgets to unwrap authenticates fine and
 * then silently discards every reply, including the PONG the heartbeat waits on,
 * so the session died on a timeout about three minutes later and reconnected
 * forever. Both transports must go through the same unwrap.
 *
 * This runs on a device because the envelope uses android.util.Base64, which
 * returns defaults under the plain JVM test runner and would pass vacuously.
 */
@RunWith(AndroidJUnit4::class)
class EncryptedEnvelopeUnwrapTest {
    private val key = "k".repeat(64)

    private fun encrypted(inner: String): Envelope =
        Protocol.decodeEnvelope(SecureEnvelope.encrypt(key, inner))

    @Test
    fun anEncryptedEnvelopeIsUnwrappedToTheMessageInsideIt() {
        for (type in listOf(MessageType.PONG, MessageType.UNPAIR, MessageType.NOTIFICATION_ACK)) {
            val inner = Protocol.json.encodeToString(
                Envelope.serializer(),
                Envelope(type = type, payload = buildJsonObject { put("id", "x") }),
            )
            assertEquals(type, unwrap(encrypted(inner), key)?.type)
        }
    }

    @Test
    fun aPlaintextEnvelopePassesThroughUntouched() {
        val envelope = Envelope(type = MessageType.AUTH_OK, payload = buildJsonObject { })
        assertEquals(MessageType.AUTH_OK, unwrap(envelope, key)?.type)
    }

    @Test
    fun anEnvelopeThisPhoneCannotDecryptIsDroppedRatherThanActedOn() {
        val sealed = encrypted("""{"version":1,"type":"PONG","payload":{}}""")
        assertNull(unwrap(sealed, "z".repeat(64)))
        assertNull(
            unwrap(
                Envelope(type = MessageType.ENCRYPTED, payload = buildJsonObject { put("nonce", "!!") }),
                key,
            ),
        )
    }

    @Test
    fun theSealedFormNeverExposesTheMessageItCarries() {
        val secret = "TOP SECRET NOTIFICATION BODY"
        val inner = Protocol.json.encodeToString(
            Envelope.serializer(),
            Envelope(type = MessageType.PONG, payload = buildJsonObject { put("m", secret) }),
        )
        val onTheWire = SecureEnvelope.encrypt(key, inner)
        assertEquals(false, onTheWire.contains(secret))
    }
}
