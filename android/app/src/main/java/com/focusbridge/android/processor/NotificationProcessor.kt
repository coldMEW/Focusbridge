package com.focusbridge.android.processor

import android.service.notification.StatusBarNotification
import com.focusbridge.android.data.local.NotificationEntity
import com.focusbridge.android.data.repository.ConfigRepository
import com.focusbridge.android.priority.Priority
import com.focusbridge.android.priority.PriorityEngine
import com.focusbridge.android.processor.parsers.DefaultParser
import java.util.UUID
import javax.inject.Inject
import kotlinx.coroutines.runBlocking

class NotificationProcessor @Inject constructor(
    private val filter: NotificationFilter,
    private val defaultParser: DefaultParser,
    private val priorityEngine: PriorityEngine,
    private val config: ConfigRepository,
) {
    fun process(sbn: StatusBarNotification): NotificationEntity? {
        if (!filter.shouldProcess(sbn)) return null
        val parsed = defaultParser.parse(sbn)
        val rules = runBlocking { ProcessorRules.load(config) }
        val searchableText = listOfNotNull(parsed.sender, parsed.message, parsed.appName, parsed.packageName)
            .joinToString(" ")
            .lowercase()
        if (rules.blockedKeywords.any { searchableText.contains(it) }) return null
        val priority = rules.overridePriority(parsed, priorityEngine.classify(parsed), searchableText)
        val masked = parsed.contentHidden || rules.privacyMode
        return NotificationEntity(
            id = "${sbn.packageName}:${sbn.id}:${sbn.postTime}:${UUID.randomUUID()}",
            appName = parsed.appName,
            packageName = parsed.packageName,
            sender = parsed.sender,
            message = parsed.message,
            timestamp = parsed.timestamp,
            receivedAt = System.currentTimeMillis(),
            priority = priority.name,
            contentHidden = masked,
        )
    }
}

private data class ProcessorRules(
    val privacyMode: Boolean,
    val blockedKeywords: List<String>,
    val priorityKeywords: List<String>,
    val favoriteContacts: List<String>,
) {
    fun overridePriority(
        parsed: ParsedNotification,
        current: Priority,
        searchableText: String,
    ): Priority {
        if (current == Priority.URGENT) return current
        val favoriteHit = favoriteContacts.any { contact ->
            parsed.sender?.lowercase()?.contains(contact) == true || searchableText.contains(contact)
        }
        val keywordHit = priorityKeywords.any { searchableText.contains(it) }
        return if (favoriteHit || keywordHit) Priority.HIGH else current
    }

    companion object {
        suspend fun load(config: ConfigRepository): ProcessorRules = ProcessorRules(
            privacyMode = config.get("privacy_mode_enabled") == "true",
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
