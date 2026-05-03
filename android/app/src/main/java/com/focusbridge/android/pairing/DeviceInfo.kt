package com.focusbridge.android.pairing

import android.os.Build

object DeviceInfo {
    val deviceName: String
        get() = listOf(Build.MANUFACTURER, Build.MODEL)
            .filter { it.isNotBlank() }
            .joinToString(" ")
}
