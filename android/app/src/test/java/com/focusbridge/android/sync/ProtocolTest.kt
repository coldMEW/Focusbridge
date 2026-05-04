package com.focusbridge.android.sync

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.encodeToJsonElement
import kotlinx.serialization.json.put

class ProtocolTest {
    @Test
    fun appInventoryUsesProtocolEnvelope() {
        val encoded = Protocol.appInventory(
            listOf(
                AppInventoryItem(
                    packageName = "com.whatsapp",
                    label = "WhatsApp",
                    category = "messaging",
                    iconDataUrl = "data:image/png;base64,abc123",
                    notificationsSeen = 2,
                    lastSeenAt = 123L,
                ),
            ),
        )

        val envelope = Protocol.json.decodeFromString(Envelope.serializer(), encoded)
        assertEquals(MessageType.APP_INVENTORY, envelope.type)
        assertTrue(encoded.contains("\"packageName\":\"com.whatsapp\""))
        assertTrue(encoded.contains("\"category\":\"messaging\""))
        assertTrue(encoded.contains("\"iconDataUrl\":\"data:image/png;base64,abc123\""))
    }

    @Test
    fun decodesRulesUpdatePayload() {
        val payload = buildJsonObject {
            put("version", 1)
            put(
                "appRules",
                Protocol.json.encodeToJsonElement(
                    listOf(
                        AppRuleUpdate(
                            packageName = "com.social.app",
                            muted = true,
                            priority = false,
                            studySafe = false,
                        ),
                    ),
                ),
            )
            put("priorityKeywords", Protocol.json.encodeToJsonElement(listOf("deadline")))
            put("blockedKeywords", Protocol.json.encodeToJsonElement(listOf("sale")))
            put("favoriteContacts", Protocol.json.encodeToJsonElement(listOf("mom")))
        }

        val decoded = Protocol.decodeRulesUpdate(payload)

        assertEquals(1, decoded.appRules.size)
        assertEquals("com.social.app", decoded.appRules.first().packageName)
        assertTrue(decoded.appRules.first().muted)
        assertEquals(listOf("deadline"), decoded.priorityKeywords)
        assertEquals(listOf("sale"), decoded.blockedKeywords)
        assertEquals(listOf("mom"), decoded.favoriteContacts)
    }

    @Test
    fun rulesAckUsesProtocolEnvelope() {
        val envelope = Protocol.decodeEnvelope(Protocol.rulesAck(appliedCount = 3))

        assertEquals(MessageType.RULES_ACK, envelope.type)
        assertTrue(Protocol.json.encodeToString(Envelope.serializer(), envelope).contains("\"appliedCount\":3"))
    }
}
