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

    @Test
    fun aChatMessageKeepsItsIdWhenNewerMessagesPushItDownTheConversation() {
        // WhatsApp shows the last few messages of a chat in each notification.
        // A new message moves the older ones to a different position; that must
        // not make them look like new messages.
        val message = ParsedNotification(
            appName = "WhatsApp",
            packageName = "com.whatsapp",
            sender = "Asha",
            message = "See you at 6",
            timestamp = 1_700_000_000_000,
            contentHidden = false,
        )

        assertEquals(
            contentStableNotificationId("chat-key", message, 0),
            contentStableNotificationId("chat-key", message, 3),
        )
    }

    @Test
    fun theSameWordsSentAtAnotherTimeAreAnotherMessage() {
        val first = ParsedNotification(
            appName = "WhatsApp",
            packageName = "com.whatsapp",
            sender = "Asha",
            message = "ok",
            timestamp = 1_700_000_000_000,
            contentHidden = false,
        )

        assertNotEquals(
            contentStableNotificationId("chat-key", first, 0),
            contentStableNotificationId("chat-key", first.copy(timestamp = 1_700_000_090_000), 0),
        )
    }
}
