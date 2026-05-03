package com.focusbridge.android.di

import android.content.Context
import androidx.room.Room
import com.focusbridge.android.data.local.ConfigDao
import com.focusbridge.android.data.local.FocusBridgeDatabase
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
        Room.databaseBuilder(context, FocusBridgeDatabase::class.java, "focusbridge.db").build()

    @Provides
    fun provideNotificationDao(db: FocusBridgeDatabase): NotificationDao = db.notifications()

    @Provides
    fun providePairingDao(db: FocusBridgeDatabase): PairingDao = db.pairings()

    @Provides
    fun provideConfigDao(db: FocusBridgeDatabase): ConfigDao = db.config()
}
