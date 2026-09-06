package com.focusbridge.android.di

import android.content.Context
import androidx.room.Room
import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase
import com.focusbridge.android.data.local.ConfigDao
import com.focusbridge.android.data.local.FocusBridgeDatabase
import com.focusbridge.android.data.local.AppRuleDao
import com.focusbridge.android.data.local.NotificationDao
import com.focusbridge.android.FocusBridgeApp
import com.focusbridge.android.data.local.PairingDao
import com.focusbridge.android.storage.DatabasePreparation
import com.focusbridge.android.security.DatabaseKeyStore
import com.focusbridge.android.storage.EncryptedDatabaseMigrator
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object DatabaseModule {
    /**
     * The application database, encrypted at rest with a key held in the Android
     * Keystore. Preparation was started when the process came up; this waits for
     * it rather than opening anything itself.
     *
     * There is deliberately no plaintext fallback. If storage cannot be unlocked
     * the app fails loudly, because quietly reopening the user's notification
     * history unencrypted would be worse than not starting.
     */
    @Provides
    @Singleton
    fun provideDatabase(@ApplicationContext context: Context): FocusBridgeDatabase {
        val passphrase = DatabasePreparation.take(context, FocusBridgeApp.DATABASE_NAME)
        return buildFrom(context, FocusBridgeApp.DATABASE_NAME, passphrase)
    }

    /** Opens an already-prepared database. The passphrase is consumed here. */
    internal fun buildFrom(
        context: Context,
        name: String,
        passphrase: ByteArray,
    ): FocusBridgeDatabase =
        try {
            Room.databaseBuilder(context, FocusBridgeDatabase::class.java, name)
                .openHelperFactory(EncryptedDatabaseMigrator.roomFactory(passphrase))
                .addMigrations(MIGRATION_1_2, MIGRATION_2_3, MIGRATION_3_4)
                .build()
        } catch (failure: Throwable) {
            passphrase.fill(0)
            throw failure
        }

    /** Prepares and opens in one step. Used by the on-device storage fixtures. */
    internal fun buildEncryptedDatabase(context: Context, name: String): FocusBridgeDatabase {
        val passphrase = EncryptedDatabaseMigrator(context.getDatabasePath(name)).prepare(DatabaseKeyStore(context))
        return buildFrom(context, name, passphrase)
    }

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

    /**
     * Adds cross-network relay routing metadata and the device-only key material
     * pinned at pairing. Defaults keep every existing LAN pairing valid and
     * relay-disabled until the phone scans a QR from a relay-enabled desktop.
     */
    private val MIGRATION_3_4 = object : Migration(3, 4) {
        override fun migrate(db: SupportSQLiteDatabase) {
            for (column in listOf(
                "relayUrl", "relayAccountKey", "relayPairId", "relayCapability",
                "desktopPublicKey", "enrollmentPsk",
            )) {
                db.execSQL("ALTER TABLE pairings ADD COLUMN $column TEXT NOT NULL DEFAULT ''")
            }
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
