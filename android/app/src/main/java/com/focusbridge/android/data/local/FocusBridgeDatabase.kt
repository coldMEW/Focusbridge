package com.focusbridge.android.data.local

import androidx.room.Database
import androidx.room.RoomDatabase

@Database(
    entities = [NotificationEntity::class, PairingEntity::class, ConfigEntity::class],
    version = 1,
    exportSchema = false,
)
abstract class FocusBridgeDatabase : RoomDatabase() {
    abstract fun notifications(): NotificationDao
    abstract fun pairings(): PairingDao
    abstract fun config(): ConfigDao
}
