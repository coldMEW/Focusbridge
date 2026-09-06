package com.focusbridge.android.security

import android.content.Context
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Injectable access to this phone's Noise static private key. Each call returns a
 * fresh copy because the native session consumes and wipes the array it is given.
 */
@Singleton
class DeviceIdentityProvider @Inject constructor(
    @ApplicationContext private val context: Context,
) {
    fun privateKey(): ByteArray = DeviceIdentityStore(context).getOrCreate()
}
