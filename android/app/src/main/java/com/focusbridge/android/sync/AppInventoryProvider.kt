package com.focusbridge.android.sync

import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.drawable.BitmapDrawable
import android.graphics.drawable.Drawable
import android.content.pm.ApplicationInfo
import android.util.Base64
import dagger.hilt.android.qualifiers.ApplicationContext
import java.io.ByteArrayOutputStream
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
                val appInfo = runCatching { manager.getApplicationInfo(packageName, 0) }.getOrNull()
                if (appInfo?.isSystemUserFacingApp(packageName) == false) return@mapNotNull null
                val label = info.loadLabel(manager)?.toString()?.takeIf { it.isNotBlank() } ?: packageName
                AppInventoryItem(
                    packageName = packageName,
                    label = label,
                    category = categorize(packageName, label),
                    iconDataUrl = info.loadIcon(manager)?.toDataUrl(),
                )
            }
            .distinctBy { it.packageName }
            .sortedWith(compareBy<AppInventoryItem> { it.category }.thenBy { it.label.lowercase() })
            .toList()
    }

    private fun categorize(packageName: String, label: String): String {
        val text = "$packageName $label".lowercase()
        return when {
            text.hasAny("whatsapp", "telegram", "signal", "messenger", "messages", "sms", "discord", "slack", "wechat", "line", "viber") -> "messaging"
            text.hasAny("instagram", "tiktok", "snapchat", "facebook", "twitter", "threads", "reddit", "linkedin", "pinterest") -> "social"
            text.hasAny("duolingo", "khan", "coursera", "udemy", "quizlet", "classroom", "canvas", "moodle", "blackboard") -> "learning"
            text.hasAny("gmail", "outlook", "mail", "proton", "yahoo") -> "email"
            text.hasAny("calendar", "meet", "zoom", "teams", "notion", "todoist", "trello") -> "school_work"
            text.hasAny("bank", "paypal", "pay", "wallet", "finance") -> "finance"
            text.hasAny("amazon", "shop", "store", "ebay", "walmart") -> "shopping"
            text.hasAny("youtube", "spotify", "netflix", "music", "video") -> "media"
            text.hasAny("android", "system", "settings", "google play") -> "system"
            else -> "other"
        }
    }

    private fun String.hasAny(vararg needles: String): Boolean = needles.any(::contains)

    private fun ApplicationInfo.isSystemUserFacingApp(packageName: String): Boolean {
        val isSystem = (flags and (ApplicationInfo.FLAG_SYSTEM or ApplicationInfo.FLAG_UPDATED_SYSTEM_APP)) != 0
        if (!isSystem) return true
        return packageName in USER_FACING_SYSTEM_PACKAGES
    }

    private fun Drawable.toDataUrl(): String? = runCatching {
        val bitmap = toBitmap(ICON_SIZE_PX, ICON_SIZE_PX)
        val out = ByteArrayOutputStream()
        bitmap.compress(Bitmap.CompressFormat.PNG, 90, out)
        "data:image/png;base64," + Base64.encodeToString(out.toByteArray(), Base64.NO_WRAP)
    }.getOrNull()

    private fun Drawable.toBitmap(width: Int, height: Int): Bitmap {
        if (this is BitmapDrawable && bitmap != null) {
            return Bitmap.createScaledBitmap(bitmap, width, height, true)
        }
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        setBounds(0, 0, canvas.width, canvas.height)
        draw(canvas)
        return bitmap
    }

    private companion object {
        const val ICON_SIZE_PX = 48
        val USER_FACING_SYSTEM_PACKAGES = setOf(
            "com.google.android.gm",
            "com.google.android.calendar",
            "com.google.android.youtube",
            "com.google.android.apps.docs",
            "com.google.android.apps.photos",
        )
    }
}
