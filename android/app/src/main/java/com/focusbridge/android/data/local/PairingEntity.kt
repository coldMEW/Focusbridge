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
    /**
     * Cross-network rendezvous. These are routing metadata and one revocable
     * transport capability; none of them can decrypt application traffic, which
     * is protected by the device-only Noise session below.
     */
    val relayUrl: String = "",
    val relayAccountKey: String = "",
    val relayPairId: String = "",
    val relayCapability: String = "",
    /** Base64 (no wrap) desktop static public key and enrollment PSK from the QR. */
    val desktopPublicKey: String = "",
    val enrollmentPsk: String = "",
) {
    fun candidateEndpoints(): List<String> =
        sequenceOf(endpoint)
            .plus(endpointCandidates.splitToSequence('|'))
            .map { it.trim() }
            .filter { it.isNotBlank() }
            .distinct()
            .toList()

    /**
     * True when this pairing carries everything needed to reach the desktop through
     * the relay. A partially populated row must fall back to LAN rather than dial a
     * relay it cannot authenticate to.
     */
    fun supportsRelay(): Boolean =
        relayUrl.isNotBlank() &&
            relayAccountKey.matches(HEX_64) &&
            relayPairId.matches(HEX_32) &&
            relayCapability.matches(CAPABILITY) &&
            desktopPublicKey.isNotBlank() &&
            enrollmentPsk.isNotBlank()

    /** `wss://host/v1/socket/{accountKey}/{pairId}/phone` for this pairing. */
    fun relaySocketUrl(): String {
        val base = relayUrl.trim().trimEnd('/').let {
            when {
                it.startsWith("wss://") -> it
                it.startsWith("https://") -> "wss://" + it.removePrefix("https://")
                // The relay rejects anything but https/wss; never downgrade silently.
                it.startsWith("ws://") || it.startsWith("http://") ->
                    throw IllegalArgumentException("Relay endpoint must use TLS")
                else -> "wss://$it"
            }
        }
        return "$base/v1/socket/$relayAccountKey/$relayPairId/phone"
    }

    private companion object {
        val HEX_64 = Regex("[a-f0-9]{64}")
        val HEX_32 = Regex("[a-f0-9]{32}")
        val CAPABILITY = Regex("[A-Za-z0-9_-]{43}")
    }
}
