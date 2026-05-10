package com.focusbridge.android.sync

import com.focusbridge.android.data.local.NotificationEntity
import com.focusbridge.android.data.repository.NotificationRepository
import com.focusbridge.android.data.repository.PairingRepository
import com.focusbridge.android.data.repository.ConfigRepository
import com.focusbridge.android.pairing.DeviceInfo
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.delay
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
            runCatching {
                if (!client.isConnected() && !isManuallyDisconnected()) {
                    connectActivePairing()
                }
            }.onFailure {
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
            for (endpoint in pairing.candidateEndpoints()) {
                client.connect(
                    pairing,
                    deviceName = DeviceInfo.deviceName,
                    endpointOverride = endpoint,
                    retryingOnFailure = true,
                )
                val connected = withTimeoutOrNull(CONNECT_TIMEOUT_MS) {
                    client.state.first { it == ConnectionState.CONNECTED }
                } != null
                if (connected) {
                    connectedNow = true
                    return@withLock
                }
            }
            client.disconnect(showDisconnected = true)
        }
        if (connectedNow && flushAfterConnect) {
            flushPending()
        }
    }

    suspend fun send(notification: NotificationEntity) {
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

    private suspend fun isManuallyDisconnected(): Boolean =
        config.get("manual_disconnect") == "true"

    private companion object {
        const val CONNECT_TIMEOUT_MS = 4_000L
        const val RECONNECT_INTERVAL_MS = 15_000L
    }
}
