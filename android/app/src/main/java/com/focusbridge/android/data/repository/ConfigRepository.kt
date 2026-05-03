package com.focusbridge.android.data.repository

import com.focusbridge.android.data.local.ConfigDao
import com.focusbridge.android.data.local.ConfigEntity
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class ConfigRepository @Inject constructor(
    private val dao: ConfigDao,
) {
    fun observe(key: String): Flow<String?> = dao.observe(key)

    suspend fun get(key: String): String? = dao.get(key)

    suspend fun set(key: String, value: String) = dao.set(ConfigEntity(key, value))
}
