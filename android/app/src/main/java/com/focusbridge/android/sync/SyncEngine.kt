package com.focusbridge.android.sync

import com.focusbridge.android.data.local.NotificationEntity
import com.focusbridge.android.data.repository.NotificationRepository
import com.focusbridge.android.data.repository.PairingRepository
import com.focusbridge.android.data.repository.ConfigRepository
import com.focusbridge.android.pairing.DeviceInfo
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.delay
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.isActive
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeoutOrNull

@Singleton
class SyncEngine @Inject constructor(
    private val pairings: PairingRepository,
    private val notifications: NotificationRepository,
    private val client: WebSocketClient,
    private val config: ConfigRepository,
) {
    private val connectMutex = Mutex()

    suspend fun maintainActivePairing() {
        while (currentCoroutineContext().isActive) {
            try {
                if (isManuallyDisconnected()) {
                    // Nothing to do until the user pairs or accepts a reconnect.
                } else if (client.isConnected()) {
                    flushPending()
                } else if (!client.isAwaitingPeer()) {
                    connectActivePairing()
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (failure: Exception) {
                android.util.Log.w("FocusBridgeSync", "Sync attempt failed; retry scheduled", failure)
                client.disconnect(showDisconnected = true)
            }
            delay(RECONNECT_INTERVAL_MS)
        }
    }

    suspend fun connectActivePairing() {
        connectActivePairing(flushAfterConnect = true)
    }

    private suspend fun connectActivePairing(flushAfterConnect: Boolean) {
        var connectedNow = false
        connectMutex.withLock {
            if (client.isConnected()) {
                connectedNow = true
                return@withLock
            }
            if (isManuallyDisconnected()) return@withLock
            val pairing = pairings.active() ?: return@withLock
            // The LAN path is preferred: it works with no Internet account, adds no
            // relay hop, and keeps working if the relay is unreachable.
            for (endpoint in pairing.candidateEndpoints()) {
                if (isManuallyDisconnected()) return@withLock
                client.connect(
                    pairing,
                    deviceName = DeviceInfo.deviceName,
                    endpointOverride = endpoint,
                    retryingOnFailure = true,
                )
                if (awaitConnected(CONNECT_TIMEOUT_MS)) {
                    connectedNow = true
                    return@withLock
                }
            }
            // No LAN route reached the desktop, so fall back to the relay. This is
            // the path that crosses networks: mobile data to a PC on home Wi-Fi,
            // isolated guest networks, and NAT in both directions.
            if (!isManuallyDisconnected() && pairing.supportsRelay()) {
                client.connect(
                    pairing,
                    deviceName = DeviceInfo.deviceName,
                    retryingOnFailure = true,
                    useRelay = true,
                )
                if (awaitConnected(RELAY_CONNECT_TIMEOUT_MS)) {
                    connectedNow = true
                    return@withLock
                }
                // Hold an attached relay socket open even though the desktop has not
                // answered: the relay reports the peer's arrival on this same socket.
                if (client.isAwaitingPeer()) return@withLock
            }
            client.disconnect(showDisconnected = true)
        }
        if (connectedNow && flushAfterConnect) {
            flushPending()
        }
    }

    suspend fun send(notification: NotificationEntity) {
        if (isManuallyDisconnected()) return
        if (!client.isConnected()) {
            connectActivePairing(flushAfterConnect = false)
        }
        if (client.send(Protocol.notification(notification))) return
        connectActivePairing(flushAfterConnect = false)
        client.send(Protocol.notification(notification))
    }

    suspend fun flushPending() {
        notifications.pending().forEach { send(it) }
    }

    /**
     * Waits for this connection attempt to settle. Only CONNECTED counts: the relay
     * transport also has to complete a device-only handshake and an authenticated
     * exchange before the desktop is really reachable.
     */
    private suspend fun awaitConnected(timeoutMs: Long): Boolean =
        withTimeoutOrNull(timeoutMs) {
            client.state.first {
                it == ConnectionState.CONNECTED ||
                    it == ConnectionState.RETRYING ||
                    it == ConnectionState.DISCONNECTED
            }
        } == ConnectionState.CONNECTED

    private suspend fun isManuallyDisconnected(): Boolean =
        client.isManuallyDisconnected() || config.get("manual_disconnect") == "true"

    private companion object {
        const val CONNECT_TIMEOUT_MS = 4_000L
        // The relay adds a round trip to Cloudflare plus a full Noise handshake and
        // an authenticated exchange, so it needs a longer budget than a LAN dial.
        const val RELAY_CONNECT_TIMEOUT_MS = 12_000L
        const val RECONNECT_INTERVAL_MS = 15_000L
    }
}
