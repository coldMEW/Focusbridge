package com.focusbridge.android.sync

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class AppInventoryProvider @Inject constructor(
    @ApplicationContext private val context: Context,
) {
    fun launchableApps(): List<AppInventoryItem> {
        val manager = context.packageManager
        val intent = Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER)
        return manager.queryIntentActivities(intent, 0)
            .asSequence()
            .mapNotNull { info ->
                val packageName = info.activityInfo?.packageName ?: return@mapNotNull null
                val label = info.loadLabel(manager)?.toString()?.takeIf { it.isNotBlank() } ?: packageName
                AppInventoryItem(
                    packageName = packageName,
                    label = label,
                    category = categorize(packageName, label),
                )
            }
            .distinctBy { it.packageName }
            .sortedWith(compareBy<AppInventoryItem> { it.category }.thenBy { it.label.lowercase() })
            .toList()
    }

    private fun categorize(packageName: String, label: String): String {
        val text = "$packageName $label".lowercase()
        return when {
            text.hasAny("whatsapp", "telegram", "signal", "messenger", "messages", "sms", "discord") -> "messaging"
            text.hasAny("gmail", "outlook", "mail", "proton") -> "email"
            text.hasAny("calendar", "meet", "zoom", "teams", "classroom", "canvas") -> "school_work"
            text.hasAny("bank", "paypal", "pay", "wallet", "finance") -> "finance"
            text.hasAny("instagram", "tiktok", "snapchat", "facebook", "twitter", "reddit") -> "social"
            text.hasAny("amazon", "shop", "store", "ebay", "walmart") -> "shopping"
            text.hasAny("youtube", "spotify", "netflix", "music", "video") -> "media"
            text.hasAny("android", "system", "settings", "google play") -> "system"
            else -> "other"
        }
    }

    private fun String.hasAny(vararg needles: String): Boolean = needles.any(::contains)
}
