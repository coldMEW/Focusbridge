package com.focusbridge.android.service

import android.service.notification.NotificationListenerService
import android.util.Log
import android.service.notification.StatusBarNotification
import com.focusbridge.android.data.repository.NotificationRepository
import com.focusbridge.android.processor.NotificationProcessor
import com.focusbridge.android.processor.stableNotificationId
import com.focusbridge.android.sync.SyncEngine
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import javax.inject.Inject

@AndroidEntryPoint
class NotificationService : NotificationListenerService() {
    @Inject lateinit var processor: NotificationProcessor
    @Inject lateinit var notifications: NotificationRepository
    @Inject lateinit var syncEngine: SyncEngine

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private companion object {
        const val TAG = "FocusBridgeSync"
    }

    override fun onNotificationPosted(sbn: StatusBarNotification) {
        val entities = processor.process(sbn)
        // Every drop on this path was silent, so "nothing arrives on the PC" gave
        // nothing to look at: no way to tell a filtered notification from one the
        // listener never saw, or from one that was saved but never sent. The
        // package name and a count are enough to tell those apart, and no message
        // content is written to the log.
        if (entities.isEmpty()) {
            Log.i(TAG, "ignored a notification from " + sbn.packageName + " (filtered or muted)")
            return
        }
        Log.i(TAG, "captured " + entities.size + " from " + sbn.packageName)
        scope.launch {
            entities.forEach { entity ->
                notifications.save(entity)
                syncEngine.send(entity)
            }
        }
    }

    override fun onNotificationRemoved(sbn: StatusBarNotification) {
        scope.launch {
            val baseId = stableNotificationId(sbn.key, sbn.packageName, sbn.id, sbn.tag)
            notifications.markSent(baseId)
            notifications.markBatchSent(baseId)
        }
    }
}
