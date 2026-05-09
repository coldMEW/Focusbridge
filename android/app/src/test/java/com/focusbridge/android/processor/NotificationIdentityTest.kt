package com.focusbridge.android.processor

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

class NotificationIdentityTest {
    @Test
    fun contentStableIdDedupesSameMailAcrossDifferentAndroidKeys() {
        val first = ParsedNotification(
            appName = "Gmail",
            packageName = "com.google.android.gm",
            sender = "Professor",
            message = "Assignment posted",
            timestamp = 1_700_000_000,
            contentHidden = false,
        )
        val second = first.copy()

        assertEquals(
            contentStableNotificationId("key-a", first, 0),
            contentStableNotificationId("key-b", second, 0),
        )
    }

    @Test
    fun contentStableIdSeparatesDistinctMessagesFromSameConversation() {
        val base = ParsedNotification(
            appName = "WhatsApp",
            packageName = "com.whatsapp",
            sender = "Asha",
            message = "First",
            timestamp = 1_700_000_000,
            contentHidden = false,
        )

        assertNotEquals(
            contentStableNotificationId("chat-key", base, 0),
            contentStableNotificationId("chat-key", base.copy(message = "Second"), 1),
        )
    }
}
