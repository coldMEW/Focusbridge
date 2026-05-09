package com.focusbridge.android.pairing

import com.focusbridge.android.data.local.PairingEntity
import com.focusbridge.android.data.repository.PairingRepository
import java.net.URI
import java.net.URLDecoder
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import javax.inject.Inject

@Serializable
data class QrPairingPayload(
    val v: Int,
    val mode: String,
    val endpoint: String,
    val endpointCandidates: List<String> = emptyList(),
    val relayUrl: String? = null,
    val devicePairId: String? = null,
    val deviceId: String,
    val pairingKey: String,
    val certFingerprint: String,
) {
    fun syncEndpoint(): String {
        val normalizedMode = mode.uppercase()
        if (normalizedMode != "CLOUD" || relayUrl.isNullOrBlank() || devicePairId.isNullOrBlank()) {
            return endpoint
        }

        val base = relayUrl.toRelayWebSocketBase()
        val key = URLEncoder.encode(pairingKey, StandardCharsets.UTF_8.name())
        return "$base/ws/$devicePairId?role=phone&pairing_key=$key"
    }

    fun syncEndpointCandidates(): List<String> {
        if (mode.uppercase() == "CLOUD") return listOf(syncEndpoint())
        return sequenceOf(endpoint)
            .plus(endpointCandidates.asSequence())
            .map { it.trim() }
            .filter { it.isNotBlank() }
            .flatMap { candidate ->
                if (candidate.startsWith("wss://")) {
                    sequenceOf("ws://${candidate.removePrefix("wss://")}", candidate)
                } else {
                    sequenceOf(candidate)
                }
            }
            .distinct()
            .toList()
    }
}

private fun String.toRelayWebSocketBase(): String {
    val trimmed = trim().trimEnd('/')
    return when {
        trimmed.startsWith("wss://") || trimmed.startsWith("ws://") -> trimmed
        trimmed.startsWith("https://") -> "wss://${trimmed.removePrefix("https://")}"
        trimmed.startsWith("http://") -> "ws://${trimmed.removePrefix("http://")}"
        else -> "wss://$trimmed"
    }
}

class PairingManager @Inject constructor(
    private val repository: PairingRepository,
) {
    private val json = Json { ignoreUnknownKeys = true }

    suspend fun consume(rawQrPayload: String): PairingEntity {
        val payload = json.decodeFromString(QrPairingPayload.serializer(), pairingPayloadFromInput(rawQrPayload))
        val candidates = payload.syncEndpointCandidates()
        val pairing = PairingEntity(
            deviceId = payload.deviceId,
            endpoint = candidates.firstOrNull() ?: payload.syncEndpoint(),
            endpointCandidates = candidates.joinToString("|"),
            pairingKey = payload.pairingKey,
            certFingerprint = payload.certFingerprint,
            mode = payload.mode.uppercase(),
        )
        repository.save(pairing)
        return pairing
    }
}

internal fun pairingPayloadFromInput(input: String): String {
    val trimmed = input.trim()
    if (!trimmed.startsWith("focusbridge://", ignoreCase = true)) return trimmed
    val uri = URI(trimmed)
    require(uri.host == "pair") { "Unsupported FocusBridge link" }
    val payload = uri.rawQuery
        ?.split('&')
        ?.firstOrNull { it.substringBefore('=') == "payload" }
        ?.substringAfter('=', "")
        ?.takeIf { it.isNotBlank() }
    return requireNotNull(payload) { "Missing pairing payload" }
        .let { URLDecoder.decode(it, StandardCharsets.UTF_8.name()) }
}
