package com.focusbridge.android.processor

import org.junit.Assert.assertEquals
import org.junit.Test

class NotificationProcessorTest {
    @Test
    fun stableNotificationIdPrefersAndroidNotificationKey() {
        assertEquals(
            "0|com.chat|-42|null|101",
            stableNotificationId("0|com.chat|-42|null|101", "com.chat", 42, null),
        )
    }

    @Test
    fun stableNotificationIdFallsBackWithoutRandomness() {
        assertEquals(
            "com.chat:42:direct",
            stableNotificationId("", "com.chat", 42, "direct"),
        )
    }
}
