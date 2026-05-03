package com.focusbridge.android.pairing

import com.focusbridge.android.data.local.PairingEntity
import com.focusbridge.android.data.repository.PairingRepository
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import javax.inject.Inject

@Serializable
data class QrPairingPayload(
    val v: Int,
    val mode: String,
    val endpoint: String,
    val relayUrl: String? = null,
    val deviceId: String,
    val pairingKey: String,
    val certFingerprint: String,
)

class PairingManager @Inject constructor(
    private val repository: PairingRepository,
) {
    private val json = Json { ignoreUnknownKeys = true }

    suspend fun consume(rawQrPayload: String): PairingEntity {
        val payload = json.decodeFromString(QrPairingPayload.serializer(), rawQrPayload)
        val pairing = PairingEntity(
            deviceId = payload.deviceId,
            endpoint = payload.endpoint,
            pairingKey = payload.pairingKey,
            certFingerprint = payload.certFingerprint,
            mode = payload.mode.uppercase(),
        )
        repository.save(pairing)
        return pairing
    }
}
