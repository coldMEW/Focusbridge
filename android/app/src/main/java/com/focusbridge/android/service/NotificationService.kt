package com.focusbridge.android.service

import android.content.ComponentName
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
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import java.util.concurrent.atomic.AtomicLong
import javax.inject.Inject

@AndroidEntryPoint
class NotificationService : NotificationListenerService() {
    @Inject lateinit var processor: NotificationProcessor
    @Inject lateinit var notifications: NotificationRepository
    @Inject lateinit var syncEngine: SyncEngine

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private val live = LiveNotifications()

    /**
     * Everything posted up to this moment has been seen. Moved forward by every
     * callback, filtered notifications included, and by recovery; the watchdog
     * compares the shade against it.
     */
    // Advanced from the main thread by callbacks and from the watchdog's thread
    // by recovery, so it only ever moves forward, atomically: a plain
    // read-then-write let a slow recovery wind it back and re-capture
    // notifications that had already been handled.
    private val handledThrough = AtomicLong(0L)

    private fun markHandledThrough(time: Long) {
        handledThrough.accumulateAndGet(time) { current, candidate -> maxOf(current, candidate) }
    }

    companion object {
        private const val TAG = "FocusBridgeSync"

        /** The listener bound in this process, while the system has it connected. */
        @Volatile
        internal var connected: NotificationService? = null
            private set

        /** When any listener in this process last received a real callback. */
        @Volatile
        internal var lastCallbackAt = 0L
            private set

        fun component(context: android.content.Context) =
            ComponentName(context, NotificationService::class.java)
    }

    override fun onListenerConnected() {
        super.onListenerConnected()
        // Only posts from now on are this listener's to catch; anything already in
        // the shade was either handled by a previous connection or predates it.
        markHandledThrough(System.currentTimeMillis())
        // What is already on screen has been seen; only a change to it is news.
        runCatching { activeNotifications }.getOrNull()?.forEach { sbn ->
            runCatching { live.remember(sbn.key, processor.signature(sbn)) }
        }
        connected = this
        Log.i(TAG, "notification listener connected")
    }

    override fun onListenerDisconnected() {
        super.onListenerDisconnected()
        if (connected === this) connected = null
        // The system does not come back on its own. Until it does, every
        // notification is lost while the phone still shows itself as connected.
        Log.w(TAG, "notification listener disconnected; asking the system to rebind it")
        runCatching { requestRebind(component(this)) }
            .onFailure { Log.w(TAG, "listener rebind request failed", it) }
    }

    override fun onDestroy() {
        if (connected === this) connected = null
        scope.cancel()
        super.onDestroy()
    }

    override fun onNotificationPosted(sbn: StatusBarNotification) {
        val now = System.currentTimeMillis()
        lastCallbackAt = now
        markHandledThrough(now)
        capture(sbn)
    }

    private fun capture(sbn: StatusBarNotification) {
        if (live.isRepeat(sbn.key, processor.signature(sbn))) {
            Log.i(TAG, "ignored an update from " + sbn.packageName + " with nothing new in it")
            return
        }
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
                // A chat notification repeats the conversation so far each time
                // a message arrives. Only the message that is new is sent; the
                // rest are already here, and re-saving one would mark it
                // pending and send it to the desktop all over again.
                if (notifications.exists(entity.id)) return@forEach
                notifications.save(entity)
                syncEngine.send(entity)
            }
        }
    }

    /**
     * Checks this listener against the shade, and captures anything it missed
     * that is still there to be read, so a gap in callbacks does not become a
     * gap in the inbox.
     */
    internal fun inspect(now: Long): ListenerWatchdog.Verdict {
        val shade = try {
            activeNotifications
        } catch (refused: SecurityException) {
            // "Disallowed call from unknown notification listener": the system
            // no longer counts this instance as its listener.
            return ListenerWatchdog.Verdict.UNRECOGNISED
        } catch (failure: RuntimeException) {
            Log.w(TAG, "could not read the notification shade", failure)
            return ListenerWatchdog.Verdict.UNRECOGNISED
        } ?: return ListenerWatchdog.Verdict.UNRECOGNISED
        val since = handledThrough.get()
        val missed = shade
            .filter { it.packageName != packageName }
            .filter { ListenerWatchdog.isMissed(it.postTime, since, now) }
            .sortedBy { it.postTime }
        if (missed.isEmpty()) return ListenerWatchdog.Verdict.HEALTHY
        Log.w(TAG, "the listener missed " + missed.size + " notification(s); recovering them from the shade")
        // Each is recovered once: from here on it counts as handled.
        markHandledThrough(missed.last().postTime)
        missed.forEach { sbn ->
            runCatching { capture(sbn) }
                .onFailure { Log.w(TAG, "could not recover a missed notification from " + sbn.packageName, it) }
        }
        return ListenerWatchdog.Verdict.MISSING_POSTS
    }

    override fun onNotificationRemoved(sbn: StatusBarNotification) {
        live.forget(sbn.key)
        scope.launch {
            val baseId = stableNotificationId(sbn.key, sbn.packageName, sbn.id, sbn.tag)
            notifications.markSent(baseId)
            notifications.markBatchSent(baseId)
        }
    }
}
