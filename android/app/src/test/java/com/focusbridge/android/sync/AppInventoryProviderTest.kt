package com.focusbridge.android.sync

import android.content.Context
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import io.mockk.*
import org.junit.Assert.*
import org.junit.Test
import org.junit.After

class AppInventoryProviderTest {
    @After fun cleanup() = unmockkConstructor(Intent::class)

    @Test fun brokenMetadataKeepsPackageInCompleteInventory() {
        mockkConstructor(Intent::class)
        every { anyConstructed<Intent>().addCategory(any()) } answers { self as Intent }
        val context = mockk<Context>()
        val manager = mockk<PackageManager>()
        val app = spyk(ApplicationInfo())
        app.packageName = "com.example.app"
        every { context.packageManager } returns manager
        every { context.packageName } returns "com.focusbridge.android"
        every { manager.queryIntentActivities(any(), any<Int>()) } returns emptyList()
        every { manager.getInstalledApplications(any<Int>()) } returns listOf(app)
        every { app.loadLabel(manager) } throws IllegalStateException("missing label")
        every { app.loadIcon(manager) } throws IllegalStateException("missing icon")
        val result = AppInventoryProvider(context).launchableApps()
        assertEquals("com.example.app", result.single().label)
        assertNull(result.single().iconDataUrl)
    }
}
