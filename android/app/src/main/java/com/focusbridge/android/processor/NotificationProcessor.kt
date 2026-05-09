package com.focusbridge.android.processor

import android.service.notification.StatusBarNotification
import com.focusbridge.android.data.local.NotificationEntity
import com.focusbridge.android.data.repository.AppRuleRepository
import com.focusbridge.android.data.repository.ConfigRepository
import com.focusbridge.android.priority.Priority
import com.focusbridge.android.priority.PriorityEngine
import com.focusbridge.android.processor.parsers.DefaultParser
import java.security.MessageDigest
import javax.inject.Inject
import kotlinx.coroutines.runBlocking

class NotificationProcessor @Inject constructor(
    private val filter: NotificationFilter,
    private val defaultParser: DefaultParser,
    private val priorityEngine: PriorityEngine,
    private val config: ConfigRepository,
    private val appRules: AppRuleRepository,
) {
    fun process(sbn: StatusBarNotification): List<NotificationEntity> {
        if (!filter.shouldProcess(sbn)) return emptyList()
        val rules = runBlocking { ProcessorRules.load(config) }
        val baseId = stableNotificationId(sbn.key, sbn.packageName, sbn.id, sbn.tag)
        return defaultParser.parseAll(sbn)
            .distinctBy { parsed ->
                listOf(parsed.packageName, parsed.sender.orEmpty(), parsed.message.orEmpty(), parsed.timestamp)
                    .joinToString("|")
            }
            .mapIndexedNotNull { index, parsed ->
                val appRule = runBlocking { appRules.get(parsed.packageName) }
                if (appRule?.muted == true) return@mapIndexedNotNull null
                val searchableText = listOfNotNull(parsed.sender, parsed.message, parsed.appName, parsed.packageName)
                    .joinToString(" ")
                    .lowercase()
                if (rules.blockedKeywords.any { searchableText.contains(it) }) return@mapIndexedNotNull null
                val priority = rules.overridePriority(
                    parsed = parsed,
                    current = priorityEngine.classify(parsed),
                    searchableText = searchableText,
                    appPriority = appRule?.priority == true,
                )
                if (rules.studyModeEnabled && appRule?.studySafe != true && priority < Priority.HIGH) {
                    return@mapIndexedNotNull null
                }
                val masked = parsed.contentHidden || rules.privacyMode
                NotificationEntity(
                    id = contentStableNotificationId(baseId, parsed, index),
                    appName = parsed.appName,
                    packageName = parsed.packageName,
                    sender = parsed.sender,
                    message = parsed.message,
                    timestamp = parsed.timestamp,
                    receivedAt = System.currentTimeMillis(),
                    priority = priority.name,
                    contentHidden = masked,
                    batchId = baseId,
                )
            }
    }
}

internal fun stableNotificationId(
    key: String?,
    packageName: String,
    id: Int,
    tag: String?,
): String = key?.takeIf { it.isNotBlank() } ?: "$packageName:$id:${tag.orEmpty()}"

@Suppress("UNUSED_PARAMETER")
internal fun contentStableNotificationId(
    baseId: String,
    parsed: ParsedNotification,
    index: Int,
): String {
    val canonical = listOf(
        parsed.packageName,
        parsed.sender.orEmpty().trim().lowercase(),
        parsed.message.orEmpty().trim().lowercase(),
        parsed.timestamp.toString(),
        index.toString(),
    ).joinToString("|")
    val digest = MessageDigest.getInstance("SHA-256")
        .digest(canonical.toByteArray(Charsets.UTF_8))
        .joinToString("") { "%02x".format(it) }
        .take(24)
    return "content:$digest"
}

private data class ProcessorRules(
    val privacyMode: Boolean,
    val studyModeEnabled: Boolean,
    val blockedKeywords: List<String>,
    val priorityKeywords: List<String>,
    val favoriteContacts: List<String>,
) {
    fun overridePriority(
        parsed: ParsedNotification,
        current: Priority,
        searchableText: String,
        appPriority: Boolean,
    ): Priority {
        if (current == Priority.URGENT) return current
        if (appPriority) return Priority.HIGH
        val favoriteHit = favoriteContacts.any { contact ->
            parsed.sender?.lowercase()?.contains(contact) == true || searchableText.contains(contact)
        }
        val keywordHit = priorityKeywords.any { searchableText.contains(it) }
        return if (favoriteHit || keywordHit) Priority.HIGH else current
    }

    companion object {
        suspend fun load(config: ConfigRepository): ProcessorRules = ProcessorRules(
            privacyMode = config.get("privacy_mode_enabled") == "true",
            studyModeEnabled = config.get("study_mode_enabled") == "true",
            blockedKeywords = config.csv("blocked_keywords"),
            priorityKeywords = config.csv("priority_keywords"),
            favoriteContacts = config.csv("favorite_contacts"),
        )
    }
}

private suspend fun ConfigRepository.csv(key: String): List<String> =
    get(key)
        ?.split(',', '\n')
        ?.map { it.trim().lowercase() }
        ?.filter { it.isNotBlank() }
        .orEmpty()
