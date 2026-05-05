package com.focusbridge.android.service

import android.service.notification.NotificationListenerService
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

    override fun onNotificationPosted(sbn: StatusBarNotification) {
        val entity = processor.process(sbn) ?: return
        scope.launch {
            notifications.save(entity)
            syncEngine.send(entity)
        }
    }

    override fun onNotificationRemoved(sbn: StatusBarNotification) {
        scope.launch {
            notifications.markSent(stableNotificationId(sbn.key, sbn.packageName, sbn.id, sbn.tag))
        }
    }
}
