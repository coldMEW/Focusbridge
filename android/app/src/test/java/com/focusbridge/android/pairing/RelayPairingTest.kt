package com.focusbridge.android.pairing

import com.focusbridge.android.data.local.PairingEntity
import com.focusbridge.android.sync.Protocol
import io.mockk.mockk
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

class RelayPairingTest {
    private val json = Json { ignoreUnknownKeys = true }

    private val accountKey = "a".repeat(64)
    private val pairId = "b".repeat(32)
    private val capability = "C".repeat(43)
    private val key32 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="

    private fun pairing(
        relayUrl: String = "https://focusbridge-relay.focusbridge.workers.dev",
        accountKey: String = this.accountKey,
        pairId: String = this.pairId,
        capability: String = this.capability,
        desktopPublicKey: String = key32,
        enrollmentPsk: String = key32,
    ) = PairingEntity(
        deviceId = "desktop",
        endpoint = "wss://192.168.1.5:9173",
        pairingKey = "k".repeat(64),
        certFingerprint = "c".repeat(64),
        relayUrl = relayUrl,
        relayAccountKey = accountKey,
        relayPairId = pairId,
        relayCapability = capability,
        desktopPublicKey = desktopPublicKey,
        enrollmentPsk = enrollmentPsk,
    )

    @Test
    fun `a complete relay block is usable`() {
        assertTrue(pairing().supportsRelay())
    }

    @Test
    fun `a pairing without relay details stays LAN only`() {
        val lanOnly = PairingEntity(
            deviceId = "desktop",
            endpoint = "wss://192.168.1.5:9173",
            pairingKey = "k".repeat(64),
            certFingerprint = "c".repeat(64),
        )
        assertFalse(lanOnly.supportsRelay())
    }

    @Test
    fun `every missing or malformed relay field disables the relay`() {
        assertFalse(pairing(relayUrl = "").supportsRelay())
        assertFalse(pairing(accountKey = "").supportsRelay())
        assertFalse(pairing(accountKey = "a".repeat(63)).supportsRelay())
        assertFalse(pairing(accountKey = "A".repeat(64)).supportsRelay())
        assertFalse(pairing(pairId = "b".repeat(31)).supportsRelay())
        assertFalse(pairing(capability = "C".repeat(42)).supportsRelay())
        assertFalse(pairing(capability = "C".repeat(42) + "!").supportsRelay())
        assertFalse(pairing(desktopPublicKey = "").supportsRelay())
        assertFalse(pairing(enrollmentPsk = "").supportsRelay())
    }

    @Test
    fun `socket url addresses this pair in the phone role`() {
        assertEquals(
            "wss://focusbridge-relay.focusbridge.workers.dev/v1/socket/$accountKey/$pairId/phone",
            pairing().relaySocketUrl(),
        )
        assertEquals(
            "wss://relay.example/v1/socket/$accountKey/$pairId/phone",
            pairing(relayUrl = "relay.example/").relaySocketUrl(),
        )
        assertEquals(
            "wss://relay.example/v1/socket/$accountKey/$pairId/phone",
            pairing(relayUrl = "wss://relay.example").relaySocketUrl(),
        )
    }

    @Test
    fun `a plaintext relay endpoint is refused rather than downgraded`() {
        for (url in listOf("ws://relay.example", "http://relay.example")) {
            try {
                pairing(relayUrl = url).relaySocketUrl()
                fail("expected $url to be refused")
            } catch (expected: IllegalArgumentException) {
                // A relay capability must never be sent over an unencrypted socket.
            }
        }
    }

    @Test
    fun `version two payloads carry relay and noise blocks`() {
        val payload = json.decodeFromString(
            QrPairingPayload.serializer(),
            """
            {"v":2,"mode":"LOCAL","endpoint":"192.168.1.5:9173",
             "endpointCandidates":["192.168.1.5:9173"],
             "deviceId":"d","pairingKey":"${"k".repeat(64)}","certFingerprint":"${"c".repeat(64)}",
             "relay":{"url":"https://r.example","accountKey":"$accountKey","pairId":"$pairId","capability":"$capability"},
             "noise":{"desktopKey":"$key32","psk":"$key32"}}
            """.trimIndent(),
        )

        assertEquals("https://r.example", payload.relay?.url)
        assertEquals(capability, payload.relay?.capability)
        assertEquals(key32, payload.noise?.desktopKey)
    }

    @Test
    fun `version one payloads still parse without relay details`() {
        val payload = json.decodeFromString(
            QrPairingPayload.serializer(),
            """
            {"v":1,"mode":"LOCAL","endpoint":"192.168.1.5:9173",
             "deviceId":"d","pairingKey":"${"k".repeat(64)}","certFingerprint":"${"c".repeat(64)}"}
            """.trimIndent(),
        )

        assertNull(payload.relay)
        assertNull(payload.noise)
    }

    @Test
    fun `preview describes a payload without saving anything`() {
        val manager = PairingManager(mockk(relaxed = true), mockk(relaxed = true))
        val lan = manager.preview(
            """
            {"v":1,"mode":"LOCAL","endpoint":"192.168.1.5:9173",
             "deviceId":"d","pairingKey":"${"k".repeat(64)}","certFingerprint":"${"c".repeat(64)}"}
            """.trimIndent(),
        )
        assertEquals("wss://192.168.1.5:9173", lan?.endpoint)
        assertEquals(false, lan?.crossNetwork)
        // The code shown to the user must match the desktop's own diagnostics.
        assertEquals("CCCCCCCCCCCC", lan?.shortFingerprint())

        val remote = manager.preview(
            """
            {"v":2,"mode":"LOCAL","endpoint":"192.168.1.5:9173",
             "deviceId":"d","pairingKey":"${"k".repeat(64)}","certFingerprint":"${"c".repeat(64)}",
             "relay":{"url":"https://r.example","accountKey":"$accountKey","pairId":"$pairId","capability":"$capability"},
             "noise":{"desktopKey":"$key32","psk":"$key32"}}
            """.trimIndent(),
        )
        assertEquals(true, remote?.crossNetwork)
    }

    @Test
    fun `preview rejects payloads that cannot be trusted`() {
        val manager = PairingManager(mockk(relaxed = true), mockk(relaxed = true))
        assertNull(manager.preview("not a payload"))
        assertNull(manager.preview("focusbridge://pair?payload=%7Bbroken"))
        assertNull(manager.preview("""{"v":1,"mode":"LOCAL"}"""))
    }

    @Test
    fun `relay control frames are parsed only when well formed`() {
        assertEquals("relay.peer_ready", Protocol.relayControlType("""{"type":"relay.peer_ready","generation":"x"}"""))
        assertEquals("relay.peer_unavailable", Protocol.relayControlType("""{"type":"relay.peer_unavailable"}"""))
        assertNull(Protocol.relayControlType("not json"))
        assertNull(Protocol.relayControlType("""{"generation":"x"}"""))
        assertNull(Protocol.relayControlType("""{"type":7}"""))
        assertNull(Protocol.relayControlType("""["relay.peer_ready"]"""))
        // Oversized control frames are rejected before parsing.
        assertNull(Protocol.relayControlType("""{"type":"relay.peer_ready","pad":"""" + "p".repeat(600) + """"}"""))
    }
}
