package com.focusbridge.android.sync

import com.focusbridge.android.data.local.NotificationEntity
import com.focusbridge.android.data.repository.NotificationRepository
import com.focusbridge.android.data.repository.PairingRepository
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class SyncEngine @Inject constructor(
    private val pairings: PairingRepository,
    private val notifications: NotificationRepository,
    private val client: WebSocketClient,
) {
    suspend fun connectActivePairing() {
        val pairing = pairings.active() ?: return
        client.connect(pairing)
        flushPending()
    }

    suspend fun send(notification: NotificationEntity) {
        if (client.send(Protocol.notification(notification))) {
            notifications.markSent(notification.id)
        }
    }

    suspend fun flushPending() {
        notifications.pending().forEach { send(it) }
    }
}
