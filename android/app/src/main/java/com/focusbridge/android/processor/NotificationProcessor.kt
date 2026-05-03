package com.focusbridge.android.processor

import android.service.notification.StatusBarNotification
import com.focusbridge.android.data.local.NotificationEntity
import com.focusbridge.android.priority.PriorityEngine
import com.focusbridge.android.processor.parsers.DefaultParser
import java.util.UUID
import javax.inject.Inject

class NotificationProcessor @Inject constructor(
    private val filter: NotificationFilter,
    private val defaultParser: DefaultParser,
    private val priorityEngine: PriorityEngine,
) {
    fun process(sbn: StatusBarNotification): NotificationEntity? {
        if (!filter.shouldProcess(sbn)) return null
        val parsed = defaultParser.parse(sbn)
        val priority = priorityEngine.classify(parsed)
        return NotificationEntity(
            id = "${sbn.packageName}:${sbn.id}:${sbn.postTime}:${UUID.randomUUID()}",
            appName = parsed.appName,
            packageName = parsed.packageName,
            sender = parsed.sender,
            message = parsed.message,
            timestamp = parsed.timestamp,
            receivedAt = System.currentTimeMillis(),
            priority = priority.name,
            contentHidden = parsed.contentHidden,
        )
    }
}
