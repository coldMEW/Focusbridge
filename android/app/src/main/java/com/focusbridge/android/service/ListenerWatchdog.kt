package com.focusbridge.android.service

/**
 * Decides whether the notification listener has gone deaf, and what to do about it.
 *
 * Android can leave a listener registered and granted while delivering it
 * nothing -- after the process is restarted under memory pressure, most often.
 * It happened for a week on a Pixel 7: the phone said Connected, 825
 * notifications were captured up to that point, and not one more reached the
 * desktop, because onNotificationPosted was simply never called again. Nothing
 * in the app noticed, since nothing in the app can see a callback that does not
 * arrive. Toggling notification access by hand brought it straight back.
 *
 * The evidence used here is the shade itself: every notification the system
 * shows was posted at a known time, and every post reaches a working listener
 * (filtered ones included -- the filtering happens after the callback). So a
 * notification posted after the last one this listener handled, and not
 * handled within a grace period, is proof that callbacks have stopped.
 */
internal object ListenerWatchdog {
    /**
     * How long a just-posted notification may wait for its callback. Generous,
     * because callbacks run on the main thread and a burst of notifications can
     * hold it for seconds; a false alarm costs a duplicate the desktop ignores.
     */
    const val GRACE_MS = 30_000L

    const val CHECK_INTERVAL_MS = 60_000L

    /** Whether a notification in the shade should already have been handled. */
    fun isMissed(postTime: Long, handledThrough: Long, now: Long): Boolean =
        postTime > handledThrough && postTime <= now - GRACE_MS

    enum class Verdict {
        /** Bound, recognised by the system, and nothing has slipped past it. */
        HEALTHY,

        /** Access is granted but no listener is bound in this process. */
        UNBOUND,

        /** Bound, but the system refuses to recognise it. */
        UNRECOGNISED,

        /** Bound and recognised, but posts are not reaching it. */
        MISSING_POSTS,
    }

    /**
     * Whether to ask the system to rebind the listener. Asking is the documented
     * remedy and costs nothing, so it is repeated at every failed check. There is
     * deliberately no stronger step: switching the component off and on could not
     * be shown to keep notification access on a real device, and losing access
     * is worse than the fault being repaired. Meanwhile anything missed is still
     * recovered from the shade at every check, so nothing is lost while waiting.
     */
    fun shouldRebind(verdict: Verdict): Boolean = verdict != Verdict.HEALTHY
}
