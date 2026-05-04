package com.focusbridge.android.data.local

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query

@Dao
interface AppRuleDao {
    @Query("SELECT * FROM app_rules WHERE packageName = :packageName")
    suspend fun get(packageName: String): AppRuleEntity?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsertAll(rules: List<AppRuleEntity>)
}
