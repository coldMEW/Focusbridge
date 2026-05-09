package com.focusbridge.android.pairing

import android.content.Context
import dagger.hilt.android.qualifiers.ApplicationContext
import java.util.UUID
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class PhoneIdentity @Inject constructor(
    @ApplicationContext context: Context,
) {
    private val prefs = context.getSharedPreferences("focusbridge_phone_identity", Context.MODE_PRIVATE)

    val installId: String
        get() {
            prefs.getString(KEY_INSTALL_ID, null)?.let { return it }
            val generated = UUID.randomUUID().toString()
            prefs.edit().putString(KEY_INSTALL_ID, generated).apply()
            return generated
        }

    val deviceName: String
        get() = DeviceInfo.deviceName

    private companion object {
        const val KEY_INSTALL_ID = "phone_install_id"
    }
}
