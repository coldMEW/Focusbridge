package com.focusbridge.android.di

import android.content.Context
import androidx.room.Room
import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase
import com.focusbridge.android.data.local.ConfigDao
import com.focusbridge.android.data.local.FocusBridgeDatabase
import com.focusbridge.android.data.local.AppRuleDao
import com.focusbridge.android.data.local.NotificationDao
import com.focusbridge.android.data.local.PairingDao
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object DatabaseModule {
    @Provides
    @Singleton
    fun provideDatabase(@ApplicationContext context: Context): FocusBridgeDatabase =
        Room.databaseBuilder(context, FocusBridgeDatabase::class.java, "focusbridge.db")
            .addMigrations(MIGRATION_1_2, MIGRATION_2_3)
            .build()

    @Provides
    fun provideNotificationDao(db: FocusBridgeDatabase): NotificationDao = db.notifications()

    @Provides
    fun providePairingDao(db: FocusBridgeDatabase): PairingDao = db.pairings()

    @Provides
    fun provideConfigDao(db: FocusBridgeDatabase): ConfigDao = db.config()

    @Provides
    fun provideAppRuleDao(db: FocusBridgeDatabase): AppRuleDao = db.appRules()

    private val MIGRATION_1_2 = object : Migration(1, 2) {
        override fun migrate(db: SupportSQLiteDatabase) {
            db.execSQL("ALTER TABLE pairings ADD COLUMN endpointCandidates TEXT NOT NULL DEFAULT ''")
        }
    }

    private val MIGRATION_2_3 = object : Migration(2, 3) {
        override fun migrate(db: SupportSQLiteDatabase) {
            db.execSQL(
                """
                CREATE TABLE IF NOT EXISTS app_rules (
                    packageName TEXT NOT NULL PRIMARY KEY,
                    muted INTEGER NOT NULL DEFAULT 0,
                    priority INTEGER NOT NULL DEFAULT 0,
                    studySafe INTEGER NOT NULL DEFAULT 0,
                    updatedAt INTEGER NOT NULL DEFAULT 0
                )
                """.trimIndent(),
            )
        }
    }
}
