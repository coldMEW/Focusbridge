package com.focusbridge.android.priority

import javax.inject.Inject

class UrgencyDetector @Inject constructor() {
    private val urgentWords = setOf("urgent", "emergency", "asap", "important", "call me")
    private val codeWords = setOf("code", "verify", "verification", "otp", "authentication", "pin")

    fun isUrgent(message: String?): Boolean {
        val text = message.orEmpty().lowercase()
        return urgentWords.any { it in text } || isTwoFactorCode(text)
    }

    fun isTwoFactorCode(message: String?): Boolean {
        val text = message.orEmpty().lowercase()
        if (codeWords.none { it in text }) return false
        return Regex("\\b\\d{4,8}\\b").containsMatchIn(text)
    }
}
