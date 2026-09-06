package com.focusbridge.android.data.local

import androidx.room.Database
import androidx.room.RoomDatabase

/**
 * The current schema version. Storage code that validates a database on disk must
 * read it from here: a bump that is not reflected there rejects the user's own
 * database as unsupported.
 */
const val FOCUSBRIDGE_SCHEMA_VERSION = 4

@Database(
    entities = [NotificationEntity::class, PairingEntity::class, ConfigEntity::class, AppRuleEntity::class],
    version = FOCUSBRIDGE_SCHEMA_VERSION,
    exportSchema = false,
)
abstract class FocusBridgeDatabase : RoomDatabase() {
    abstract fun notifications(): NotificationDao
    abstract fun pairings(): PairingDao
    abstract fun config(): ConfigDao
    abstract fun appRules(): AppRuleDao
}
