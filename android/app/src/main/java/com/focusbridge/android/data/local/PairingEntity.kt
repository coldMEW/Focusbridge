package com.focusbridge.android.data.local

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = "pairings")
data class PairingEntity(
    @PrimaryKey val deviceId: String,
    val endpoint: String,
    val endpointCandidates: String = "",
    val pairingKey: String,
    val certFingerprint: String,
    val mode: String = "LOCAL",
    val createdAt: Long = System.currentTimeMillis(),
    val active: Boolean = true,
) {
    fun candidateEndpoints(): List<String> =
        sequenceOf(endpoint)
            .plus(endpointCandidates.splitToSequence('|'))
            .map { it.trim() }
            .filter { it.isNotBlank() }
            .distinct()
            .toList()
}
