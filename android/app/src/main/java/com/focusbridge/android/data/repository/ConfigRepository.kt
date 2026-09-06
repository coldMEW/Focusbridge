package com.focusbridge.android.data.repository

import com.focusbridge.android.data.local.ConfigDao
import com.focusbridge.android.data.local.ConfigEntity
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.distinctUntilChanged
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class ConfigRepository @Inject constructor(
    private val dao: ConfigDao,
) {
    fun observeAppLock(): Flow<Map<String, String>> = dao.observeAppLock()
        .map { entries -> entries.associate { it.key to it.value } }.distinctUntilChanged()

    suspend fun setAll(values: Map<String, String>) = dao.setAll(values.map { ConfigEntity(it.key, it.value) })

    fun observe(key: String): Flow<String?> = dao.observe(key)

    suspend fun get(key: String): String? = dao.get(key)

    suspend fun set(key: String, value: String) = dao.set(ConfigEntity(key, value))
}
