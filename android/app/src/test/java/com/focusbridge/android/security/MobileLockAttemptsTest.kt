package com.focusbridge.android.security

import com.focusbridge.android.data.local.ConfigDao
import com.focusbridge.android.data.local.ConfigEntity
import com.focusbridge.android.data.repository.ConfigRepository
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MobileLockAttemptsTest {
    private class MemoryDao : ConfigDao {
        val values = mutableMapOf<String, String>()
        var reservations = 0
        override fun observe(key: String): Flow<String?> = flowOf(values[key])
        override fun observeAppLock(): Flow<List<ConfigEntity>> = flowOf(emptyList())
        override suspend fun get(key: String) = values[key]
        override suspend fun set(entity: ConfigEntity) {
            if (entity.key == "mobile_lock_next_attempt" && entity.value != "0") reservations++
            values[entity.key] = entity.value
        }
        override suspend fun setAll(entities: List<ConfigEntity>) { entities.forEach { set(it) } }
    }

    @Test fun failureBlocksRecoveryAndSurvivesRecreation() = runBlocking {
        val dao = MemoryDao()
        val config = ConfigRepository(dao)
        val salt = MobileAppLockCrypto.newSalt()
        val hash = MobileAppLockCrypto.hashSecret("1234", salt)
        assertFalse(MobileLockAttempts(config).verify("wrong", salt, hash))
        assertTrue(dao.values.getValue("mobile_lock_next_attempt").toLong() > System.currentTimeMillis())
        assertFalse(MobileLockAttempts(config).verify("1234", salt, hash))
        assertEquals(1, dao.reservations)
        dao.values["mobile_lock_next_attempt"] = "0"
        assertTrue(MobileLockAttempts(config).verify("1234", salt, hash))
        assertEquals("0", dao.values["mobile_lock_next_attempt"])
    }

    @Test fun concurrentFailuresReserveOnlyOneAttempt() = runBlocking {
        val dao = MemoryDao()
        val attempts = MobileLockAttempts(ConfigRepository(dao))
        val salt = MobileAppLockCrypto.newSalt()
        val hash = MobileAppLockCrypto.hashSecret("1234", salt)
        val results = List(8) { async { attempts.verify("wrong", salt, hash) } }.awaitAll()
        assertTrue(results.none { it })
        assertEquals(1, dao.reservations)
    }
}
