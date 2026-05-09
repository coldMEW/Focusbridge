package com.focusbridge.android.processor

import android.app.Notification
import android.service.notification.StatusBarNotification
import javax.inject.Inject

class NotificationFilter @Inject constructor() {
    fun shouldProcess(sbn: StatusBarNotification): Boolean {
        val notification = sbn.notification ?: return false
        if (sbn.isOngoing) return false
        if ((notification.flags and Notification.FLAG_ONGOING_EVENT) != 0) return false
        if ((notification.flags and Notification.FLAG_FOREGROUND_SERVICE) != 0) return false
        if ((notification.flags and Notification.FLAG_GROUP_SUMMARY) != 0) return false
        if (sbn.packageName == "android" || sbn.packageName == "com.android.systemui") return false
        val title = notification.extras.getCharSequence(Notification.EXTRA_TITLE)?.toString()
        val text = notification.extras.getCharSequence(Notification.EXTRA_TEXT)?.toString()
        return !title.isNullOrBlank() || !text.isNullOrBlank()
    }
}
