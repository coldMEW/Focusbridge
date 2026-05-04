package com.focusbridge.android.sync

import com.focusbridge.android.data.local.NotificationEntity
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.encodeToJsonElement
import kotlinx.serialization.json.put

@Serializable
data class Envelope(
    val version: Int = 1,
    val type: MessageType,
    val payload: JsonElement,
)

@Serializable
enum class MessageType {
    AUTH,
    AUTH_OK,
    AUTH_FAILED,
    NOTIFICATION,
    NOTIFICATION_BATCH,
    DISMISSAL,
    DISMISSAL_BATCH,
    PING,
    PONG,
    STATUS,
    APP_INVENTORY,
    RULES_UPDATE,
    RULES_ACK,
    DESKTOP_ACTION,
    UNPAIR,
    ENCRYPTED,
}

@Serializable
data class AuthPayload(
    val pairingKey: String,
    val deviceId: String,
    val deviceName: String,
    val role: String = "phone",
)

@Serializable
data class AppInventoryItem(
    val packageName: String,
    val label: String,
    val category: String,
    val iconDataUrl: String? = null,
    val notificationsSeen: Int = 0,
    val lastSeenAt: Long = 0,
)

@Serializable
data class AppInventoryPayload(
    val apps: List<AppInventoryItem>,
)

@Serializable
data class AppRuleUpdate(
    val packageName: String,
    val muted: Boolean = false,
    val priority: Boolean = false,
    val studySafe: Boolean = false,
)

@Serializable
data class RulesUpdatePayload(
    val version: Int = 1,
    val appRules: List<AppRuleUpdate> = emptyList(),
    val priorityKeywords: List<String> = emptyList(),
    val blockedKeywords: List<String> = emptyList(),
    val favoriteContacts: List<String> = emptyList(),
)

object Protocol {
    val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
    }

    fun auth(pairingKey: String, deviceId: String, deviceName: String): String =
        json.encodeToString(
            Envelope.serializer(),
            Envelope(
                type = MessageType.AUTH,
                payload = json.encodeToJsonElement(
                    AuthPayload.serializer(),
                    AuthPayload(pairingKey, deviceId, deviceName),
                ),
            ),
        )

    fun notification(notification: NotificationEntity): String =
        json.encodeToString(
            Envelope.serializer(),
            Envelope(
                type = MessageType.NOTIFICATION,
                payload = buildJsonObject {
                    put("id", notification.id)
                    put("appName", notification.appName)
                    put("packageName", notification.packageName)
                    if (notification.sender == null) put("sender", JsonNull) else put("sender", notification.sender)
                    if (notification.message == null) put("message", JsonNull) else put("message", notification.message)
                    put("timestamp", notification.timestamp)
                    put("priority", notification.priority)
                    put("contentHidden", notification.contentHidden)
                    if (notification.batchId != null) put("batchId", notification.batchId)
                },
            ),
        )

    fun dismissal(id: String): String =
        json.encodeToString(
            Envelope.serializer(),
            Envelope(
                type = MessageType.DISMISSAL,
                payload = buildJsonObject { put("id", id) },
            ),
        )

    fun appInventory(apps: List<AppInventoryItem>): String =
        json.encodeToString(
            Envelope.serializer(),
            Envelope(
                type = MessageType.APP_INVENTORY,
                payload = json.encodeToJsonElement(
                    AppInventoryPayload.serializer(),
                    AppInventoryPayload(apps),
                ),
            ),
        )

    fun decodeEnvelope(text: String): Envelope =
        json.decodeFromString(Envelope.serializer(), text)

    fun decodeRulesUpdate(payload: JsonElement): RulesUpdatePayload =
        json.decodeFromJsonElement(RulesUpdatePayload.serializer(), payload)

    fun rulesAck(appliedCount: Int): String =
        json.encodeToString(
            Envelope.serializer(),
            Envelope(
                type = MessageType.RULES_ACK,
                payload = buildJsonObject {
                    put("applied", true)
                    put("appliedCount", appliedCount)
                },
            ),
        )
}
