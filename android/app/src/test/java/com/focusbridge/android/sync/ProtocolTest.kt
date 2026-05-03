package com.focusbridge.android.sync

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ProtocolTest {
    @Test
    fun appInventoryUsesProtocolEnvelope() {
        val encoded = Protocol.appInventory(
            listOf(
                AppInventoryItem(
                    packageName = "com.whatsapp",
                    label = "WhatsApp",
                    category = "messaging",
                    notificationsSeen = 2,
                    lastSeenAt = 123L,
                ),
            ),
        )

        val envelope = Protocol.json.decodeFromString(Envelope.serializer(), encoded)
        assertEquals(MessageType.APP_INVENTORY, envelope.type)
        assertTrue(encoded.contains("\"packageName\":\"com.whatsapp\""))
        assertTrue(encoded.contains("\"category\":\"messaging\""))
    }
}
