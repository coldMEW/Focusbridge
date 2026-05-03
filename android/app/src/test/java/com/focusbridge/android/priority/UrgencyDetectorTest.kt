package com.focusbridge.android.priority

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class UrgencyDetectorTest {
    private val detector = UrgencyDetector()

    @Test
    fun detectsTwoFactorCodes() {
        assertTrue(detector.isTwoFactorCode("Your verification code is 123456"))
        assertFalse(detector.isTwoFactorCode("Call me at 1234567890"))
    }

    @Test
    fun detectsUrgentLanguage() {
        assertTrue(detector.isUrgent("urgent: call me"))
        assertFalse(detector.isUrgent("liked your photo"))
    }
}
