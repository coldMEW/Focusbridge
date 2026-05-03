package com.focusbridge.android.processor

data class ParsedNotification(
    val appName: String,
    val packageName: String,
    val sender: String?,
    val message: String?,
    val timestamp: Long,
    val contentHidden: Boolean,
)
