package com.focusbridge.android.data.repository

import com.focusbridge.android.data.local.NotificationDao
import com.focusbridge.android.data.local.NotificationEntity
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class NotificationRepository @Inject constructor(
    private val dao: NotificationDao,
) {
    fun observeRecent(): Flow<List<NotificationEntity>> = dao.observeRecent()

    suspend fun save(notification: NotificationEntity) = dao.upsert(notification)

    suspend fun markSent(id: String) = dao.setStatus(id, "SENT")

    suspend fun pending(): List<NotificationEntity> = dao.pending()
}
