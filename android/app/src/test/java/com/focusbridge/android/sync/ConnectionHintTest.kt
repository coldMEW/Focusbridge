package com.focusbridge.android.sync

import com.focusbridge.android.data.local.PairingEntity
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ConnectionHintTest {
    private fun pairing(relay: Boolean) = PairingEntity(
        deviceId = "desktop",
        endpoint = "wss://192.168.1.5:9173",
        pairingKey = "k".repeat(64),
        certFingerprint = "c".repeat(64),
        relayUrl = if (relay) "https://relay.example" else "",
        relayAccountKey = if (relay) "a".repeat(64) else "",
        relayPairId = if (relay) "b".repeat(32) else "",
        relayCapability = if (relay) "C".repeat(43) else "",
        desktopPublicKey = if (relay) "AAAA" else "",
        enrollmentPsk = if (relay) "BBBB" else "",
    )

    @Test
    fun `no pairing asks the user to scan a QR`() {
        assertTrue(connectionHint(null, ConnectionState.DISCONNECTED).contains("scan the QR"))
    }

    @Test
    fun `a LAN-only pairing that cannot connect explains why instead of showing an address`() {
        for (state in listOf(ConnectionState.DISCONNECTED, ConnectionState.RETRYING, ConnectionState.CONNECTING)) {
            val hint = connectionHint(pairing(relay = false), state)
            // The saved address is the least useful thing to show here: it looks
            // like a network fault when the fix is a re-scan.
            assertTrue(hint, hint.contains("same Wi-Fi"))
            assertTrue(hint, hint.contains("cross-network sync"))
            assertTrue(hint, !hint.contains("192.168.1.5"))
        }
    }

    @Test
    fun `a relay pairing that is down says it will recover on its own`() {
        val hint = connectionHint(pairing(relay = true), ConnectionState.RETRYING)
        assertTrue(hint, hint.contains("any network"))
        assertTrue(hint, !hint.contains("same Wi-Fi"))
    }

    @Test
    fun `a connected LAN pairing still shows the address it is using`() {
        assertEquals(
            "wss://192.168.1.5:9173",
            connectionHint(pairing(relay = false), ConnectionState.CONNECTED),
        )
    }

    @Test
    fun `a connected relay pairing says it works from other networks`() {
        val hint = connectionHint(pairing(relay = true), ConnectionState.CONNECTED)
        assertTrue(hint, hint.contains("other networks"))
    }
}
