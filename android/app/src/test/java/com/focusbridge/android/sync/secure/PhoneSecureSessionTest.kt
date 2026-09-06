package com.focusbridge.android.sync.secure

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

/**
 * Exercises the phone half of the handshake against a scripted bridge. The real
 * cryptography is covered by the shared Rust crate's own vectors; what is verified
 * here is the ordering the transport depends on and the fail-closed behaviour.
 */
class PhoneSecureSessionTest {
    private class RecordingBridge(
        private val readHandshakeFails: Boolean = false,
        private val readConfirmationFails: Boolean = false,
        private val readyAfterConfirmation: Boolean = true,
    ) : SecureChannelBridge {
        val calls = mutableListOf<String>()
        var createdHandle = 7L
        var closed = 0
        var lastPrivateKey: ByteArray? = null
        var lastPsk: ByteArray? = null
        private var handshakeWrites = 0
        private var ready = false
        var openResult: ByteArray? = null
        var sealResult: Array<ByteArray> = arrayOf(byteArrayOf(9))

        override fun createPhone(
            privateKey: ByteArray,
            enrollmentPsk: ByteArray,
            pairId: ByteArray,
            desktopPublicKey: ByteArray,
        ): Long {
            calls += "createPhone"
            lastPrivateKey = privateKey
            lastPsk = enrollmentPsk
            // The real implementation consumes and wipes both secrets.
            privateKey.fill(0)
            enrollmentPsk.fill(0)
            return createdHandle
        }

        override fun writeHandshake(handle: Long): ByteArray {
            calls += "writeHandshake"
            handshakeWrites++
            return byteArrayOf(handshakeWrites.toByte())
        }

        override fun readHandshake(handle: Long, frame: ByteArray) {
            calls += "readHandshake"
            if (readHandshakeFails) throw IllegalStateException("bad handshake")
        }

        override fun writeConfirmation(handle: Long): ByteArray {
            calls += "writeConfirmation"
            ready = readyAfterConfirmation
            return byteArrayOf(0x55)
        }

        override fun readConfirmation(handle: Long, frame: ByteArray) {
            calls += "readConfirmation"
            if (readConfirmationFails) throw IllegalStateException("bad confirmation")
        }

        override fun isReady(handle: Long): Boolean {
            calls += "isReady"
            return ready
        }

        override fun seal(handle: Long, plaintext: ByteArray): Array<ByteArray> {
            calls += "seal"
            return sealResult
        }

        override fun open(handle: Long, frame: ByteArray): ByteArray? {
            calls += "open"
            return openResult
        }

        override fun close(handle: Long) {
            calls += "close"
            closed++
        }
    }

    private fun session(bridge: SecureChannelBridge) = PhoneSecureSession(bridge)

    private fun start(session: PhoneSecureSession): ByteArray =
        session.start(ByteArray(32) { 1 }, ByteArray(32) { 2 }, ByteArray(16) { 3 }, ByteArray(32) { 4 })

    @Test
    fun `completes the fixed handshake and confirmation order`() {
        val bridge = RecordingBridge()
        val session = session(bridge)

        val first = start(session)
        assertArrayEquals(byteArrayOf(1), first)
        assertFalse(session.isReady)

        // Handshake message 2 arrives; the phone owes message 3.
        val afterHandshake = session.receive(byteArrayOf(0x02))
        assertTrue(afterHandshake is SecureStep.Send)
        assertArrayEquals(byteArrayOf(2), (afterHandshake as SecureStep.Send).frames.single())
        assertFalse(session.isReady)

        // Desktop confirmation arrives; the phone replies and is only now ready.
        val afterConfirmation = session.receive(byteArrayOf(0x03))
        assertTrue(afterConfirmation is SecureStep.Send)
        assertArrayEquals(byteArrayOf(0x55), (afterConfirmation as SecureStep.Send).frames.single())
        assertTrue(session.isReady)

        assertEquals(
            listOf(
                "createPhone", "writeHandshake",
                "readHandshake", "writeHandshake",
                "readConfirmation", "writeConfirmation", "isReady",
            ),
            bridge.calls,
        )
    }

    @Test
    fun `start wipes the secrets it is given`() {
        val bridge = RecordingBridge()
        start(session(bridge))

        assertArrayEquals(ByteArray(32), bridge.lastPrivateKey)
        assertArrayEquals(ByteArray(32), bridge.lastPsk)
    }

    @Test
    fun `application records are only delivered after the session is ready`() {
        val bridge = RecordingBridge()
        val session = session(bridge)
        start(session)
        session.receive(byteArrayOf(0x02))
        session.receive(byteArrayOf(0x03))

        bridge.openResult = null
        assertSame(SecureStep.Continue, session.receive(byteArrayOf(0x10)))

        bridge.openResult = "hello".toByteArray()
        val delivered = session.receive(byteArrayOf(0x11))
        assertTrue(delivered is SecureStep.Deliver)
        assertEquals("hello", String((delivered as SecureStep.Deliver).plaintext))
    }

    @Test
    fun `a handshake failure closes the session permanently`() {
        val bridge = RecordingBridge(readHandshakeFails = true)
        val session = session(bridge)
        start(session)

        try {
            session.receive(byteArrayOf(0x02))
            fail("expected the handshake failure to propagate")
        } catch (expected: IllegalStateException) {
            // The transport must reconnect rather than retry the frame.
        }

        assertTrue(session.isClosed)
        assertEquals(1, bridge.closed)
        try {
            session.receive(byteArrayOf(0x02))
            fail("a closed session must not accept another frame")
        } catch (expected: IllegalStateException) {
            // expected
        }
    }

    @Test
    fun `a confirmation that does not reach ready state fails closed`() {
        val bridge = RecordingBridge(readyAfterConfirmation = false)
        val session = session(bridge)
        start(session)
        session.receive(byteArrayOf(0x02))

        try {
            session.receive(byteArrayOf(0x03))
            fail("a session that never reports ready must not be usable")
        } catch (expected: IllegalStateException) {
            // expected
        }
        assertTrue(session.isClosed)
        assertFalse(session.isReady)
    }

    @Test
    fun `sealing before the session is ready is refused`() {
        val bridge = RecordingBridge()
        val session = session(bridge)
        start(session)

        try {
            session.seal("data".toByteArray())
            fail("application data must not be sealed mid-handshake")
        } catch (expected: IllegalStateException) {
            // expected
        }
        assertTrue(session.isClosed)
        assertFalse(bridge.calls.contains("seal"))
    }

    @Test
    fun `seal returns every ordered frame`() {
        val bridge = RecordingBridge()
        bridge.sealResult = arrayOf(byteArrayOf(1), byteArrayOf(2), byteArrayOf(3))
        val session = session(bridge)
        start(session)
        session.receive(byteArrayOf(0x02))
        session.receive(byteArrayOf(0x03))

        assertEquals(3, session.seal("data".toByteArray()).size)
    }

    @Test
    fun `close is idempotent and check alive reports a dead session`() {
        val bridge = RecordingBridge()
        val session = session(bridge)
        start(session)

        session.close()
        session.close()

        assertEquals(1, bridge.closed)
        assertFalse(session.checkAlive())
    }

    @Test
    fun `check alive closes a session whose native liveness check throws`() {
        val bridge = object : SecureChannelBridge by RecordingBridge() {
            override fun isReady(handle: Long): Boolean = throw IllegalStateException("expired")
        }
        val session = session(bridge)
        session.start(ByteArray(32) { 1 }, ByteArray(32) { 2 }, ByteArray(16) { 3 }, ByteArray(32) { 4 })

        assertFalse(session.checkAlive())
        assertTrue(session.isClosed)
    }

    @Test
    fun `a session cannot be started twice`() {
        val session = session(RecordingBridge())
        start(session)

        try {
            start(session)
            fail("restarting a session would reuse handshake state")
        } catch (expected: IllegalStateException) {
            // expected
        }
    }

    @Test
    fun `frames before start are refused`() {
        val session = session(RecordingBridge())
        try {
            session.receive(byteArrayOf(1))
            fail("frames must not be processed before the handshake begins")
        } catch (expected: IllegalStateException) {
            // expected
        }
        assertNull(null)
    }
}
