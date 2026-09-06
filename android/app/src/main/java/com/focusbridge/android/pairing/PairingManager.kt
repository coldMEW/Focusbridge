package com.focusbridge.android.pairing

import com.focusbridge.android.data.local.PairingEntity
import com.focusbridge.android.sync.WebSocketClient
import com.focusbridge.android.data.repository.PairingRepository
import java.net.URI
import java.net.URLDecoder
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import javax.inject.Inject

/**
 * Cross-network rendezvous, present from QR version 2 onwards. `url`, `accountKey`
 * and `pairId` are routing metadata; `capability` is a revocable transport token
 * that authorizes routing only. None of them can read application traffic.
 */
@Serializable
data class QrRelayBlock(
    val url: String,
    val accountKey: String,
    val pairId: String,
    val capability: String,
)

/**
 * Device-only key material for the Noise session: the desktop's static public key
 * this phone pins, and the single-pairing enrollment pre-shared key. Neither is
 * ever sent to the relay.
 */
@Serializable
data class QrNoiseBlock(
    val desktopKey: String,
    val psk: String,
)

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
    val relay: QrRelayBlock? = null,
    val noise: QrNoiseBlock? = null,
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
            .map { candidate ->
                when {
                    candidate.startsWith("wss://") -> candidate
                    candidate.startsWith("ws://") -> "wss://${candidate.removePrefix("ws://")}"
                    else -> "wss://$candidate"
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

/**
 * What a pairing payload would do, shown to the user before anything is saved.
 *
 * A pairing hands a PC the ability to read this phone's notifications, so it must
 * never be established by an intent alone: any installed app, or any web page,
 * can send a `focusbridge://pair` link.
 */
data class PairingPreview(
    val endpoint: String,
    val certificateFingerprint: String,
    val crossNetwork: Boolean,
) {
    /** The first characters of the fingerprint the desktop shows in its diagnostics. */
    fun shortFingerprint(): String = certificateFingerprint.take(12).uppercase()
}

class PairingManager @Inject constructor(
    private val repository: PairingRepository,
    private val client: WebSocketClient,
) {
    private val json = Json { ignoreUnknownKeys = true }

    /** Parses a payload for display without saving anything. Null when unusable. */
    fun preview(rawQrPayload: String): PairingPreview? = runCatching {
        val payload = json.decodeFromString(
            QrPairingPayload.serializer(),
            pairingPayloadFromInput(rawQrPayload),
        )
        PairingPreview(
            endpoint = payload.syncEndpointCandidates().firstOrNull() ?: payload.endpoint,
            certificateFingerprint = payload.certFingerprint,
            crossNetwork = payload.relay != null && payload.noise != null,
        )
    }.getOrNull()

    suspend fun consume(rawQrPayload: String): PairingEntity {
        val payload = json.decodeFromString(QrPairingPayload.serializer(), pairingPayloadFromInput(rawQrPayload))
        val candidates = payload.syncEndpointCandidates()
        // Relay details are accepted only as a matched set. A half-populated block
        // would leave the phone dialing a relay it cannot authenticate to, so it is
        // dropped and the pairing stays LAN-only.
        val relay = payload.relay?.takeIf { payload.noise != null }
        val noise = payload.noise?.takeIf { relay != null }
        val pairing = PairingEntity(
            deviceId = payload.deviceId,
            endpoint = candidates.firstOrNull() ?: payload.syncEndpoint(),
            endpointCandidates = candidates.joinToString("|"),
            pairingKey = payload.pairingKey,
            certFingerprint = payload.certFingerprint,
            mode = payload.mode.uppercase(),
            relayUrl = relay?.url.orEmpty(),
            relayAccountKey = relay?.accountKey.orEmpty(),
            relayPairId = relay?.pairId.orEmpty(),
            relayCapability = relay?.capability.orEmpty(),
            desktopPublicKey = noise?.desktopKey.orEmpty(),
            enrollmentPsk = noise?.psk.orEmpty(),
        )
        repository.save(pairing)
        client.acceptReconnectRequest()
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
