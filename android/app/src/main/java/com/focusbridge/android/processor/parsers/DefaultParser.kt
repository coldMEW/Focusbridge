package com.focusbridge.android.processor.parsers

import android.app.Notification
import android.content.pm.PackageManager
import android.os.Bundle
import android.service.notification.StatusBarNotification
import com.focusbridge.android.processor.NotificationParser
import com.focusbridge.android.processor.ParsedNotification
import javax.inject.Inject

class DefaultParser @Inject constructor(
    private val packageManager: PackageManager,
) : NotificationParser {
    override fun canParse(packageName: String): Boolean = true

    fun parseAll(sbn: StatusBarNotification): List<ParsedNotification> {
        val extras = sbn.notification.extras
        val messages = parseMessageBundles(extras)
        if (messages.isEmpty()) return listOf(parse(sbn))

        val fallbackTitle = extras.getCharSequence(Notification.EXTRA_TITLE)?.toString()
        val appName = appName(sbn.packageName)
        return messages.mapNotNull { message ->
            val text = message.text.takeIf { it.isNotBlank() } ?: return@mapNotNull null
            ParsedNotification(
                appName = appName,
                packageName = sbn.packageName,
                sender = message.sender ?: fallbackTitle,
                message = text,
                timestamp = message.timestamp.takeIf { it > 0L } ?: sbn.postTime,
                contentHidden = false,
            )
        }.ifEmpty { listOf(parse(sbn)) }
    }

    override fun parse(sbn: StatusBarNotification): ParsedNotification {
        val extras = sbn.notification.extras
        val title = extras.getCharSequence(Notification.EXTRA_TITLE)?.toString()
        val text = extras.getCharSequence(Notification.EXTRA_TEXT)?.toString()
        val bigText = extras.getCharSequence(Notification.EXTRA_BIG_TEXT)?.toString()
        return ParsedNotification(
            appName = appName(sbn.packageName),
            packageName = sbn.packageName,
            sender = title,
            message = bigText ?: text,
            timestamp = sbn.postTime,
            contentHidden = title.isNullOrBlank() && text.isNullOrBlank() && bigText.isNullOrBlank(),
        )
    }

    private fun appName(packageName: String): String =
        runCatching {
            val info = packageManager.getApplicationInfo(packageName, 0)
            packageManager.getApplicationLabel(info).toString()
        }.getOrDefault(packageName)

    @Suppress("DEPRECATION")
    private fun parseMessageBundles(extras: Bundle): List<ConversationMessage> =
        extras.getParcelableArray(Notification.EXTRA_MESSAGES)
            ?.mapNotNull { it as? Bundle }
            ?.mapNotNull { bundle ->
                val text = bundle.getCharSequence("text")?.toString()?.trim().orEmpty()
                if (text.isBlank()) return@mapNotNull null
                ConversationMessage(
                    sender = bundle.getCharSequence("sender")?.toString(),
                    text = text,
                    timestamp = bundle.getLong("time", 0L),
                )
            }
            .orEmpty()

    private data class ConversationMessage(
        val sender: String?,
        val text: String,
        val timestamp: Long,
    )
}
