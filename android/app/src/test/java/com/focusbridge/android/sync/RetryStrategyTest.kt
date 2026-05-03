package com.focusbridge.android.sync

import org.junit.Assert.assertEquals
import org.junit.Test

class RetryStrategyTest {
    @Test
    fun delayBacksOffAndCaps() {
        val retry = RetryStrategy()
        assertEquals(1_000L, retry.delayMillis(0))
        assertEquals(2_000L, retry.delayMillis(1))
        assertEquals(64_000L, retry.delayMillis(8))
    }
}
