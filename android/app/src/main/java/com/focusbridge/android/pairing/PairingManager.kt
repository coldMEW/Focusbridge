package com.focusbridge.android.pairing

import com.focusbridge.android.data.local.PairingEntity
import com.focusbridge.android.sync.WebSocketClient
import com.focusbridge.android.data.repository.PairingRepository
import java.net.URI
import java.net.URLDecoder
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.util.Base64
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
        val payload = parsePairingPayload(rawQrPayload, json)
        PairingPreview(
            endpoint = payload.syncEndpointCandidates().firstOrNull() ?: payload.endpoint,
            certificateFingerprint = payload.certFingerprint,
            crossNetwork = payload.relay != null && payload.noise != null,
        )
    }.getOrNull()

    suspend fun consume(rawQrPayload: String): PairingEntity {
        val payload = parsePairingPayload(rawQrPayload, json)
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
        // Scanning the code is the permission; the next connection must not ask
        // for it again.
        client.notePairedByUser()
        client.acceptReconnectRequest()
        return pairing
    }
}

/**
 * Reads whatever the user gave us: a scanned link in either encoding, or JSON
 * pasted by hand.
 *
 * The QR carries the compact form because the JSON one was too dense to scan --
 * 963 characters is a 117-module symbol, under three pixels a module at the size
 * a monitor shows it. Typed and pasted payloads stay JSON, so both are accepted.
 */
internal fun parsePairingPayload(input: String, json: Json): QrPairingPayload {
    compactParam(input)?.let { return decodeCompactPayload(it) }
    return json.decodeFromString(QrPairingPayload.serializer(), pairingPayloadFromInput(input))
}

private fun queryParam(input: String, name: String): String? {
    val trimmed = input.trim()
    if (!trimmed.startsWith("focusbridge://", ignoreCase = true)) return null
    val uri = URI(trimmed)
    require(uri.host == "pair") { "Unsupported FocusBridge link" }
    return uri.rawQuery
        ?.split('&')
        ?.firstOrNull { it.substringBefore('=') == name }
        ?.substringAfter('=', "")
        ?.takeIf { it.isNotBlank() }
}

private fun compactParam(input: String): String? = runCatching { queryParam(input, "c") }.getOrNull()

internal fun pairingPayloadFromInput(input: String): String {
    val trimmed = input.trim()
    if (!trimmed.startsWith("focusbridge://", ignoreCase = true)) return trimmed
    val payload = queryParam(trimmed, "payload")
    return requireNotNull(payload) { "Missing pairing payload" }
        .let { URLDecoder.decode(it, StandardCharsets.UTF_8.name()) }
}

/**
 * The compact QR encoding, version 3. The desktop writes the same layout; the
 * field order below is the wire format and the two have to stay identical.
 *
 * ```text
 *   0      version, 3
 *   1      flags; bit 0 set when the relay and Noise blocks are present
 *   2..18  device id, the raw UUID
 *  18..50  pairing key
 *  50..82  certificate fingerprint
 *  82      number of LAN candidates, then 4 bytes of IPv4 and 2 of port each
 *          when bit 0 is set: relay URL length, the URL, then the account key
 *          (32), pair id (16), capability (32), desktop key (32) and PSK (32)
 * ```
 */
internal fun decodeCompactPayload(encoded: String): QrPairingPayload {
    val bytes = try {
        Base64.getUrlDecoder().decode(encoded.trimEnd('='))
    } catch (failure: IllegalArgumentException) {
        throw IllegalArgumentException("This is not a FocusBridge pairing code", failure)
    }
    val reader = CompactReader(bytes)
    require(reader.byte() == COMPACT_VERSION) {
        "This pairing code was made by a newer version of FocusBridge"
    }
    val flags = reader.byte().toInt()
    val deviceId = reader.take(16).toUuidString()
    val pairingKey = reader.take(32).toHex()
    val certFingerprint = reader.take(32).toHex()

    val candidateCount = reader.byte().toInt() and 0xFF
    val candidates = (0 until candidateCount).map {
        val raw = reader.take(6)
        val host = (0 until 4).joinToString(".") { index -> (raw[index].toInt() and 0xFF).toString() }
        val port = ((raw[4].toInt() and 0xFF) shl 8) or (raw[5].toInt() and 0xFF)
        "wss://$host:$port"
    }
    require(candidates.isNotEmpty()) { "This pairing code has no address to connect to" }

    var relay: QrRelayBlock? = null
    var noise: QrNoiseBlock? = null
    if (flags and COMPACT_FLAG_RELAY != 0) {
        val url = String(reader.take(reader.byte().toInt() and 0xFF), StandardCharsets.UTF_8)
        val accountKey = reader.take(32).toHex()
        val pairId = reader.take(16).toHex()
        // The capability is a bearer token the relay compares as text, so it has
        // to come back in exactly the alphabet the relay issued it in.
        val capability = Base64.getUrlEncoder().withoutPadding().encodeToString(reader.take(32))
        relay = QrRelayBlock(url = url, accountKey = accountKey, pairId = pairId, capability = capability)
        noise = QrNoiseBlock(
            desktopKey = Base64.getEncoder().encodeToString(reader.take(32)),
            psk = Base64.getEncoder().encodeToString(reader.take(32)),
        )
    }
    // Trailing bytes mean this is not the payload it claims to be.
    require(reader.finished()) { "This pairing code is damaged; ask the PC for a new one" }

    return QrPairingPayload(
        v = if (relay != null) 2 else 1,
        mode = "local",
        endpoint = candidates.first(),
        endpointCandidates = candidates,
        deviceId = deviceId,
        pairingKey = pairingKey,
        certFingerprint = certFingerprint,
        relay = relay,
        noise = noise,
    )
}

private const val COMPACT_VERSION: Byte = 3
private const val COMPACT_FLAG_RELAY = 1

/** Every read is bounds-checked, so a truncated code is refused, not half-read. */
private class CompactReader(private val bytes: ByteArray) {
    private var at = 0

    fun take(length: Int): ByteArray {
        require(length >= 0 && at + length <= bytes.size) {
            "This pairing code is incomplete; ask the PC for a new one"
        }
        return bytes.copyOfRange(at, at + length).also { at += length }
    }

    fun byte(): Byte = take(1)[0]

    fun finished(): Boolean = at == bytes.size
}

private fun ByteArray.toHex(): String = joinToString("") { "%02x".format(it) }

private fun ByteArray.toUuidString(): String {
    val hex = toHex()
    return "${hex.substring(0, 8)}-${hex.substring(8, 12)}-${hex.substring(12, 16)}-" +
        "${hex.substring(16, 20)}-${hex.substring(20, 32)}"
}
