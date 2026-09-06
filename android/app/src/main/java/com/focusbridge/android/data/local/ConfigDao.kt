package com.focusbridge.android.data.local

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import kotlinx.coroutines.flow.Flow

@Dao
interface ConfigDao {
    @Query("SELECT * FROM config WHERE `key` IN ('mobile_lock_enabled', 'mobile_lock_salt', 'mobile_lock_hash', 'mobile_lock_recovery_question', 'mobile_lock_recovery_salt', 'mobile_lock_recovery_hash')")
    fun observeAppLock(): Flow<List<ConfigEntity>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun setAll(entities: List<ConfigEntity>)

    @Query("SELECT value FROM config WHERE `key` = :key")
    fun observe(key: String): Flow<String?>

    @Query("SELECT value FROM config WHERE `key` = :key")
    suspend fun get(key: String): String?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun set(entity: ConfigEntity)
}
