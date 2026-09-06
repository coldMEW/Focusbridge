package com.focusbridge.android

import android.app.Application
import com.focusbridge.android.storage.DatabasePreparation
import dagger.hilt.android.HiltAndroidApp

@HiltAndroidApp
class FocusBridgeApp : Application() {
    override fun onCreate() {
        super.onCreate()
        // Begin unlocking storage immediately, on a background thread, so the
        // first component that needs the database rarely has to wait for it.
        DatabasePreparation.start(this, DATABASE_NAME)
    }

    companion object {
        const val DATABASE_NAME = "focusbridge.db"
    }
}
