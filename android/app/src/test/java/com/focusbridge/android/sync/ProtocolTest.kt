package com.focusbridge.android.sync

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.encodeToJsonElement
import kotlinx.serialization.json.put

class ProtocolTest {
    @Test
    fun authIncludesStablePhoneInstallId() {
        val envelope = Protocol.decodeEnvelope(
            Protocol.auth(
                pairingKey = "pairing-key",
                deviceId = "qr-session-id",
                deviceName = "Pixel 8",
                phoneInstallId = "phone-install-id",
            ),
        )

        assertEquals(MessageType.AUTH, envelope.type)
        val encoded = Protocol.json.encodeToString(Envelope.serializer(), envelope)
        assertTrue(encoded.contains("\"deviceId\":\"qr-session-id\""))
        assertTrue(encoded.contains("\"phoneInstallId\":\"phone-install-id\""))
        assertTrue(encoded.contains("\"deviceName\":\"Pixel 8\""))
    }

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

    @Test
    fun pingUsesProtocolEnvelope() {
        val envelope = Protocol.decodeEnvelope(Protocol.ping())

        assertEquals(MessageType.PING, envelope.type)
    }

    @Test
    fun disconnectRequestUsesUnpairEnvelope() {
        val envelope = Protocol.decodeEnvelope(Protocol.disconnectRequest())

        assertEquals(MessageType.UNPAIR, envelope.type)
        assertTrue(Protocol.json.encodeToString(Envelope.serializer(), envelope).contains("\"reason\":\"manual_disconnect\""))
    }

    @Test
    fun statusUsesProtocolEnvelope() {
        val envelope = Protocol.decodeEnvelope(
            Protocol.status(
                studyModeEnabled = true,
                notificationsCaptured = 4,
                notificationsSent = 3,
                uptime = 120,
            ),
        )

        assertEquals(MessageType.STATUS, envelope.type)
        val encoded = Protocol.json.encodeToString(Envelope.serializer(), envelope)
        assertTrue(encoded.contains("\"studyModeEnabled\":true"))
        assertTrue(encoded.contains("\"notificationsCaptured\":4"))
        assertTrue(encoded.contains("\"notificationsSent\":3"))
    }

    @Test
    fun decodesNotificationAckPayload() {
        val ack = Protocol.decodeNotificationAck(
            buildJsonObject {
                put("id", "notification-1")
                put("accepted", true)
                put("serverTime", 1234L)
            },
        )

        assertEquals("notification-1", ack.id)
        assertTrue(ack.accepted)
        assertEquals(1234L, ack.serverTime)
    }

    @Test
    fun decodesDesktopReconnectActionPayload() {
        val action = Protocol.decodeDesktopAction(
            buildJsonObject {
                put("action", "reconnect_request")
                put("deviceId", "phone-1")
                put("requestedAt", 99L)
            },
        )

        assertEquals("reconnect_request", action.action)
        assertEquals("phone-1", action.deviceId)
        assertEquals(99L, action.requestedAt)
    }
}
