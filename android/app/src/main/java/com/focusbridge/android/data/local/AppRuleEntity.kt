package com.focusbridge.android.data.local

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = "app_rules")
data class AppRuleEntity(
    @PrimaryKey val packageName: String,
    val muted: Boolean = false,
    val priority: Boolean = false,
    val studySafe: Boolean = false,
    val updatedAt: Long = 0,
)
