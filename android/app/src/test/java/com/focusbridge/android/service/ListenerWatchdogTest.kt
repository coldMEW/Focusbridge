package com.focusbridge.android.service

import com.focusbridge.android.service.ListenerWatchdog.Verdict
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ListenerWatchdogTest {
    private val now = 1_000_000L

    @Test
    fun aNotificationPostedAfterTheLastCallbackAndNeverHandledIsMissed() {
        // The week-long outage: posts kept landing in the shade, and the last
        // callback was days before them.
        assertTrue(ListenerWatchdog.isMissed(postTime = now - 60_000, handledThrough = now - 600_000, now = now))
    }

    @Test
    fun aNotificationStillWaitingForItsCallbackIsNotMissedYet() {
        assertFalse(ListenerWatchdog.isMissed(postTime = now - 2_000, handledThrough = now - 600_000, now = now))
    }

    @Test
    fun aNotificationAlreadyHandledIsNotMissed() {
        assertFalse(ListenerWatchdog.isMissed(postTime = now - 60_000, handledThrough = now - 30_000, now = now))
        assertFalse(ListenerWatchdog.isMissed(postTime = now - 60_000, handledThrough = now - 60_000, now = now))
    }

    @Test
    fun aHealthyListenerIsLeftAlone() {
        assertFalse(ListenerWatchdog.shouldRebind(Verdict.HEALTHY))
    }

    @Test
    fun everyFailedCheckAsksForARebind() {
        for (verdict in listOf(Verdict.UNBOUND, Verdict.UNRECOGNISED, Verdict.MISSING_POSTS)) {
            assertTrue(ListenerWatchdog.shouldRebind(verdict))
        }
    }
}
