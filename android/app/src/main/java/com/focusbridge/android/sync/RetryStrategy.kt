package com.focusbridge.android.sync

import javax.inject.Inject
import kotlin.math.min

class RetryStrategy @Inject constructor() {
    fun delayMillis(attempt: Int): Long {
        val capped = min(attempt, 6)
        return 1_000L shl capped
    }
}
