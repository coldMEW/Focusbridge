package com.focusbridge.android.processor.parsers

import android.app.Notification
import android.content.pm.PackageManager
import android.service.notification.StatusBarNotification
import com.focusbridge.android.processor.NotificationParser
import com.focusbridge.android.processor.ParsedNotification
import javax.inject.Inject

class DefaultParser @Inject constructor(
    private val packageManager: PackageManager,
) : NotificationParser {
    override fun canParse(packageName: String): Boolean = true

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
}
