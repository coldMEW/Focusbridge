package com.focusbridge.android.pairing

import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The compact pairing code is written by the desktop in Rust and read here in
 * Kotlin, so the two implementations can drift without either side failing its
 * own tests. The link below is not hand-written: it is the exact output of the
 * desktop encoder for a known pairing, so if the wire format changes on one side
 * this test fails rather than the pairing failing on someone's phone.
 */
class CompactPairingPayloadTest {
    private val json = Json { ignoreUnknownKeys = true }

    private val desktopWrittenLink =
        "focusbridge://pair?c=AwE_K5waTV5Ke4ydDh8qO0xdqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq" +
            "qqq7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7uwLAqAQXI9WsFBABI9UxaHR0cHM6Ly9mb2N1c2" +
            "JyaWRnZS1yZWxheS5mb2N1c2JyaWRnZS53b3JrZXJzLmRldszMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMz" +
            "MzMzMzM3d3d3d3d3d3d3d3d3d3d3QcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHCQkJCQkJCQkJ" +
            "CQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkLCwsLCwsLCwsLCwsLCwsLCwsLCwsLCwsLCwsLCwsLCw"

    @Test fun readsEveryFieldTheDesktopWrote() {
        val payload = parsePairingPayload(desktopWrittenLink, json)

        assertEquals("3f2b9c1a-4d5e-4a7b-8c9d-0e1f2a3b4c5d", payload.deviceId)
        assertEquals("a".repeat(64), payload.pairingKey)
        assertEquals("b".repeat(64), payload.certFingerprint)
        assertEquals(
            listOf("wss://192.168.4.23:9173", "wss://172.20.16.1:9173"),
            payload.endpointCandidates,
        )
        assertEquals("wss://192.168.4.23:9173", payload.endpoint)

        val relay = requireNotNull(payload.relay)
        assertEquals("https://focusbridge-relay.focusbridge.workers.dev", relay.url)
        assertEquals("c".repeat(64), relay.accountKey)
        assertEquals("d".repeat(32), relay.pairId)
        // The relay compares the capability as text, so it has to come back in
        // the same alphabet it was issued in, character for character.
        assertEquals("BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc", relay.capability)

        val noise = requireNotNull(payload.noise)
        assertEquals("CQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQk=", noise.desktopKey)
        assertEquals("CwsLCwsLCwsLCwsLCwsLCwsLCwsLCwsLCwsLCwsLCws=", noise.psk)
    }

    @Test fun theCompactCodeIsShortEnoughToScan() {
        // The reason this encoding exists. The JSON link ran to 963 characters,
        // which is a 117-module symbol and too fine for a phone camera at the
        // size a monitor shows it.
        assertTrue(
            "the pairing link is ${desktopWrittenLink.length} characters",
            desktopWrittenLink.length <= 425,
        )
    }

    @Test fun previewShowsTheSameDetailsAsTheJsonForm() {
        val manager = PairingManager(
            repository = io.mockk.mockk(relaxed = true),
            client = io.mockk.mockk(relaxed = true),
        )

        val preview = requireNotNull(manager.preview(desktopWrittenLink))

        assertEquals("wss://192.168.4.23:9173", preview.endpoint)
        assertEquals("b".repeat(64), preview.certificateFingerprint)
        assertTrue(preview.crossNetwork)
    }

    @Test fun aDamagedCodeIsRefusedRatherThanHalfRead() {
        val manager = PairingManager(
            repository = io.mockk.mockk(relaxed = true),
            client = io.mockk.mockk(relaxed = true),
        )

        assertNull(manager.preview(desktopWrittenLink.dropLast(4)))
        assertNull(manager.preview(desktopWrittenLink + "AAAA"))
        assertNull(manager.preview("focusbridge://pair?c=not-a-code"))
    }

    @Test fun aPastedJsonPayloadStillWorks() {
        // Manual entry is the fallback when a camera cannot be used at all, so
        // the older readable form has to keep parsing.
        val jsonPayload = """
            {"v":1,"mode":"local","endpoint":"wss://10.0.0.5:9173",
             "deviceId":"desktop","pairingKey":"${"a".repeat(64)}",
             "certFingerprint":"${"b".repeat(64)}"}
        """.trimIndent()

        val payload = parsePairingPayload(jsonPayload, json)

        assertEquals("desktop", payload.deviceId)
        assertNull(payload.relay)
    }
}
