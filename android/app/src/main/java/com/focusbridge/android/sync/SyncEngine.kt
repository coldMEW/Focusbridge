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
        client.connect(pairing)
        val connected = withTimeoutOrNull(CONNECT_TIMEOUT_MS) {
            client.state.first { it == ConnectionState.CONNECTED }
        } != null
        if (connected) {
            flushPending()
        }
    }

    suspend fun send(notification: NotificationEntity) {
        if (client.send(Protocol.notification(notification))) {
            notifications.markSent(notification.id)
        }
    }

    suspend fun flushPending() {
        notifications.pending().forEach { send(it) }
    }

    private companion object {
        const val CONNECT_TIMEOUT_MS = 10_000L
    }
}
