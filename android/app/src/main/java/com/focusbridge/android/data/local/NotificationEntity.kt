package com.focusbridge.android.data.local

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = "notifications")
data class NotificationEntity(
    @PrimaryKey val id: String,
    val appName: String,
    val packageName: String,
    val sender: String?,
    val message: String?,
    val timestamp: Long,
    val receivedAt: Long,
    val status: String = "PENDING",
    val priority: String = "NORMAL",
    val contentHidden: Boolean = false,
    val batchId: String? = null,
)
