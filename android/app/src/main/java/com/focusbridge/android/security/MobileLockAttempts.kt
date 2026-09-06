package com.focusbridge.android.security

import com.focusbridge.android.data.repository.ConfigRepository
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

@Singleton
class MobileLockAttempts @Inject constructor(private val config: ConfigRepository) {
    private val mutex = Mutex()

    // Reserve before hashing so cancellation/process death cannot erase a failed attempt.
    suspend fun verify(secret: String, salt: String?, hash: String?): Boolean = mutex.withLock {
        val now = System.currentTimeMillis()
        val next = config.get("mobile_lock_next_attempt")?.toLongOrNull() ?: 0L
        if (now < next) return@withLock false
        config.set("mobile_lock_next_attempt", (now + 30_000L).toString())
        val valid = withContext(Dispatchers.Default) { MobileAppLockCrypto.verify(secret, salt, hash) }
        if (valid) config.set("mobile_lock_next_attempt", "0")
        valid
    }
}
