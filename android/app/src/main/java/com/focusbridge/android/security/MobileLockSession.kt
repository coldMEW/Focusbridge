package com.focusbridge.android.security

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

class MobileLockSession {
    var unlocked by mutableStateOf(false)
        private set
    var generation = 0
        private set

    fun relock() {
        generation++
        unlocked = false
    }

    fun unlock(attemptGeneration: Int) {
        if (attemptGeneration == generation) unlocked = true
    }
}
