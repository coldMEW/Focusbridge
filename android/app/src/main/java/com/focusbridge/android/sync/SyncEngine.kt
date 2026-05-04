package com.focusbridge.android.sync

import com.focusbridge.android.data.local.NotificationEntity
import com.focusbridge.android.data.repository.NotificationRepository
import com.focusbridge.android.data.repository.PairingRepository
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeoutOrNull

@Singleton
class SyncEngine @Inject constructor(
    private val pairings: PairingRepository,
    private val notifications: NotificationRepository,
    private val client: WebSocketClient,
) {
    private val connectMutex = Mutex()

    suspend fun maintainActivePairing() {
        while (true) {
            if (!client.isConnected()) {
                connectActivePairing()
            }
            delay(RECONNECT_INTERVAL_MS)
        }
    }

    suspend fun connectActivePairing() {
        var connectedNow = false
        connectMutex.withLock {
            if (client.isConnected()) {
                connectedNow = true
                return@withLock
            }
            val pairing = pairings.active() ?: return@withLock
            for (endpoint in pairing.candidateEndpoints()) {
                client.connect(pairing, endpointOverride = endpoint)
                val connected = withTimeoutOrNull(CONNECT_TIMEOUT_MS) {
                    client.state.first { it == ConnectionState.CONNECTED }
                } != null
                if (connected) {
                    connectedNow = true
                    return@withLock
                }
            }
        }
        if (connectedNow) {
            flushPending()
        }
    }

    suspend fun send(notification: NotificationEntity) {
        if (!client.isConnected()) {
            connectActivePairing()
        }
        if (client.send(Protocol.notification(notification))) {
            notifications.markSent(notification.id)
            return
        }
        connectActivePairing()
        if (client.send(Protocol.notification(notification))) {
            notifications.markSent(notification.id)
        }
    }

    suspend fun flushPending() {
        notifications.pending().forEach { send(it) }
    }

    private companion object {
        const val CONNECT_TIMEOUT_MS = 4_000L
        const val RECONNECT_INTERVAL_MS = 15_000L
    }
}
