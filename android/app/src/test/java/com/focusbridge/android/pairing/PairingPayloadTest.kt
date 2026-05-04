package com.focusbridge.android.pairing

import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Test

class PairingPayloadTest {
    private val json = Json { ignoreUnknownKeys = true }

    @Test
    fun localPairingUsesAdvertisedDesktopEndpoint() {
        val payload = json.decodeFromString(
            QrPairingPayload.serializer(),
            """
            {
              "v": 1,
              "mode": "local",
              "endpoint": "192.168.1.24:9173",
              "endpointCandidates": ["192.168.1.24:9173", "10.0.0.4:9173"],
              "deviceId": "desktop-1",
              "pairingKey": "abc123",
              "certFingerprint": "ffff"
            }
            """.trimIndent(),
        )

        assertEquals("192.168.1.24:9173", payload.syncEndpoint())
        assertEquals(
            listOf("192.168.1.24:9173", "10.0.0.4:9173"),
            payload.syncEndpointCandidates(),
        )
    }

    @Test
    fun localWssPairingIncludesPlaintextFallbacksForMigration() {
        val payload = json.decodeFromString(
            QrPairingPayload.serializer(),
            """
            {
              "v": 1,
              "mode": "local",
              "endpoint": "wss://192.168.1.24:9173",
              "endpointCandidates": ["wss://192.168.1.24:9173", "wss://10.0.0.4:9173"],
              "deviceId": "desktop-1",
              "pairingKey": "abc123",
              "certFingerprint": "ffff"
            }
            """.trimIndent(),
        )

        assertEquals(
            listOf(
                "wss://192.168.1.24:9173",
                "ws://192.168.1.24:9173",
                "wss://10.0.0.4:9173",
                "ws://10.0.0.4:9173",
            ),
            payload.syncEndpointCandidates(),
        )
    }

    @Test
    fun cloudPairingBuildsRelayWebSocketEndpointForPhoneRole() {
        val payload = json.decodeFromString(
            QrPairingPayload.serializer(),
            """
            {
              "v": 1,
              "mode": "cloud",
              "endpoint": "192.168.1.24:9173",
              "relayUrl": "https://relay.focusbridge.test",
              "devicePairId": "pair_123",
              "deviceId": "desktop-1",
              "pairingKey": "abc 123",
              "certFingerprint": "ffff"
            }
            """.trimIndent(),
        )

        assertEquals(
            "wss://relay.focusbridge.test/ws/pair_123?role=phone&pairing_key=abc+123",
            payload.syncEndpoint(),
        )
    }
}
