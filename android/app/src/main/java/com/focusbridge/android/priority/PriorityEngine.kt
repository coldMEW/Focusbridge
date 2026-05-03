package com.focusbridge.android.priority

import com.focusbridge.android.processor.ParsedNotification
import javax.inject.Inject

enum class Priority {
    LOW,
    NORMAL,
    HIGH,
    URGENT,
}

class PriorityEngine @Inject constructor(
    private val urgencyDetector: UrgencyDetector,
) {
    fun classify(notification: ParsedNotification): Priority {
        val packageName = notification.packageName
        val message = notification.message
        if (urgencyDetector.isTwoFactorCode(message)) return Priority.URGENT
        if (urgencyDetector.isUrgent(message)) return Priority.HIGH
        if (packageName.contains("instagram") || packageName.contains("tiktok")) return Priority.LOW
        return Priority.NORMAL
    }
}
