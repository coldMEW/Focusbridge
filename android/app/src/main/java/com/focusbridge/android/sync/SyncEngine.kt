package com.focusbridge.android.sync

import com.focusbridge.android.data.local.NotificationEntity
import com.focusbridge.android.data.repository.NotificationRepository
import com.focusbridge.android.data.repository.PairingRepository
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withTimeoutOrNull

@Singleton
class SyncEngine @Inject constructor(
    private val pairings: PairingRepository,
    private val notifications: NotificationRepository,
    private val client: WebSocketClient,
) {
    suspend fun connectActivePairing() {
        val pairing = pairings.active() ?: return
        for (endpoint in pairing.candidateEndpoints()) {
            client.connect(pairing, endpointOverride = endpoint)
            val connected = withTimeoutOrNull(CONNECT_TIMEOUT_MS) {
                client.state.first { it == ConnectionState.CONNECTED }
            } != null
            if (connected) {
                flushPending()
                return
            }
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
    }
}
