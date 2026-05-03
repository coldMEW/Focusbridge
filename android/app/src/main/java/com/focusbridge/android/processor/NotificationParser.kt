package com.focusbridge.android.processor

import android.service.notification.StatusBarNotification

interface NotificationParser {
    fun canParse(packageName: String): Boolean
    fun parse(sbn: StatusBarNotification): ParsedNotification
}
