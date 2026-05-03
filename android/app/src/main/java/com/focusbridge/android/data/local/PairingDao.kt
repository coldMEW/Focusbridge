package com.focusbridge.android.data.local

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import kotlinx.coroutines.flow.Flow

@Dao
interface PairingDao {
    @Query("SELECT * FROM pairings WHERE active = 1 LIMIT 1")
    fun observeActive(): Flow<PairingEntity?>

    @Query("SELECT * FROM pairings WHERE active = 1 LIMIT 1")
    suspend fun active(): PairingEntity?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(pairing: PairingEntity)

    @Query("UPDATE pairings SET active = 0")
    suspend fun deactivateAll()
}
