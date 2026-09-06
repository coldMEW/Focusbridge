package com.focusbridge.android.sync.secure

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Proves the shared Rust engine actually loads and runs on this device's ABI.
 *
 * The JVM unit tests exercise the transport state machine against a fake bridge;
 * only this test can show that `libfocusbridge_secure_channel_jni.so` was built
 * for the right architecture, was packaged into the APK, survived minification,
 * and that its JNI signatures match the Kotlin object. A missing or mismatched
 * library fails here rather than on a user's first cross-network connection.
 *
 * The phone role is the only one the bridge exposes, so a complete handshake
 * needs the desktop peer. What is verified here is library loading, input
 * validation, secret wiping, and handle lifetime — the parts that are purely
 * local to the phone.
 */
@RunWith(AndroidJUnit4::class)
class NativeSecureChannelInstrumentedTest {
    private fun key(fill: Byte) = ByteArray(32) { fill }
    private fun pairId(fill: Byte = 3) = ByteArray(16) { fill }

    /** A valid, unrelated Curve25519 public key: the base point times a scalar. */
    private fun desktopKey() = key(4)

    private fun createSession(): Long =
        NativeSecureChannel.createPhone(key(1), key(2), pairId(), desktopKey())

    @Test
    fun theNativeLibraryLoadsAndProducesAHandshakeMessage() {
        val handle = createSession()
        try {
            assertNotEquals(0L, handle)
            val first = NativeSecureChannel.writeHandshake(handle)
            // Noise XX message 1 is `-> e`. Under a psk modifier, `e` is followed by
            // MixKey(e.public_key), so the empty payload is already encrypted:
            // 32 ephemeral bytes plus a 16-byte ChaChaPoly tag.
            assertEquals(48, first.size)
            assertFalse(first.all { it == 0.toByte() })
            // The session is not usable until the desktop has been authenticated.
            assertFalse(NativeSecureChannel.isReady(handle))
        } finally {
            NativeSecureChannel.close(handle)
        }
    }

    @Test
    fun everyHandshakeUsesFreshEphemeralKeys() {
        val first: ByteArray
        val second: ByteArray
        val a = createSession()
        try {
            first = NativeSecureChannel.writeHandshake(a)
        } finally {
            NativeSecureChannel.close(a)
        }
        val b = createSession()
        try {
            second = NativeSecureChannel.writeHandshake(b)
        } finally {
            NativeSecureChannel.close(b)
        }
        assertEquals(48, first.size)
        assertFalse("handshake keys must not repeat", first.contentEquals(second))
    }

    @Test
    fun creatingASessionWipesTheSecretsItIsGiven() {
        val privateKey = key(1)
        val psk = key(2)
        val handle = NativeSecureChannel.createPhone(privateKey, psk, pairId(), desktopKey())
        try {
            assertArrayEquals(ByteArray(32), privateKey)
            assertArrayEquals(ByteArray(32), psk)
        } finally {
            NativeSecureChannel.close(handle)
        }
    }

    @Test
    fun malformedSecretsAreRejectedAndStillWiped() {
        val shortKey = ByteArray(16) { 1 }
        val psk = key(2)
        try {
            NativeSecureChannel.createPhone(shortKey, psk, pairId(), desktopKey())
            fail("a 16-byte private key must be rejected")
        } catch (expected: Throwable) {
            // Native code wipes both arrays even on the invalid-length path.
            assertArrayEquals(ByteArray(16), shortKey)
            assertArrayEquals(ByteArray(32), psk)
        }
    }

    @Test
    fun malformedPairIdsAndDesktopKeysAreRejected() {
        for (case in listOf(
            { NativeSecureChannel.createPhone(key(1), key(2), ByteArray(15), desktopKey()) },
            { NativeSecureChannel.createPhone(key(1), key(2), pairId(), ByteArray(31)) },
            // An all-zero pair id or key is never a real pairing.
            { NativeSecureChannel.createPhone(key(1), key(2), ByteArray(16), desktopKey()) },
            { NativeSecureChannel.createPhone(key(1), key(2), pairId(), ByteArray(32)) },
        )) {
            try {
                val handle = case()
                NativeSecureChannel.close(handle)
                fail("expected the malformed input to be rejected")
            } catch (expected: Throwable) {
                // expected
            }
        }
    }

    @Test
    fun applicationDataCannotBeSealedBeforeTheSessionIsReady() {
        val handle = createSession()
        try {
            NativeSecureChannel.seal(handle, "notification".toByteArray())
            fail("sealing mid-handshake must fail closed")
        } catch (expected: Throwable) {
            // expected
        } finally {
            NativeSecureChannel.close(handle)
        }
    }

    @Test
    fun aGarbageHandshakeFrameRetiresTheSession() {
        val handle = createSession()
        NativeSecureChannel.writeHandshake(handle)
        try {
            NativeSecureChannel.readHandshake(handle, ByteArray(48) { 0x41 })
            fail("an unauthenticated handshake frame must be rejected")
        } catch (expected: Throwable) {
            // expected
        }
        // The handle is retired by the failure, so it must not be reusable.
        try {
            NativeSecureChannel.isReady(handle)
            fail("a retired handle must not be usable")
        } catch (expected: Throwable) {
            // expected
        }
        NativeSecureChannel.close(handle)
    }

    @Test
    fun oversizedFramesAreRejectedBeforeBeingCopied() {
        val handle = createSession()
        NativeSecureChannel.writeHandshake(handle)
        try {
            NativeSecureChannel.readHandshake(handle, ByteArray(4096))
            fail("a handshake frame beyond the 256-byte cap must be rejected")
        } catch (expected: Throwable) {
            // expected
        } finally {
            NativeSecureChannel.close(handle)
        }
    }

    @Test
    fun unknownHandlesAreRejectedAndCloseIsIdempotent() {
        try {
            NativeSecureChannel.writeHandshake(999_999L)
            fail("an unknown handle must be rejected")
        } catch (expected: Throwable) {
            // expected
        }
        val handle = createSession()
        NativeSecureChannel.close(handle)
        NativeSecureChannel.close(handle)
        NativeSecureChannel.close(999_999L)
    }

    @Test
    fun handlesAreBoundedAndNeverReused() {
        val handles = mutableListOf<Long>()
        try {
            repeat(32) { handles += createSession() }
            assertEquals(32, handles.distinct().size)
            try {
                createSession()
                fail("the registry must refuse a 33rd concurrent session")
            } catch (expected: Throwable) {
                // expected
            }
            val reclaimed = handles.removeAt(0)
            NativeSecureChannel.close(reclaimed)
            val replacement = createSession()
            handles += replacement
            assertTrue("handles must not be recycled", replacement > reclaimed)
        } finally {
            handles.forEach { NativeSecureChannel.close(it) }
        }
    }
}
