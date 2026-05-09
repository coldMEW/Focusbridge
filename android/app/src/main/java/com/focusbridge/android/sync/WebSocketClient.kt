package com.focusbridge.android.sync

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import com.focusbridge.android.MainActivity
import com.focusbridge.android.R
import com.focusbridge.android.data.local.AppRuleEntity
import com.focusbridge.android.data.local.PairingEntity
import com.focusbridge.android.data.repository.AppRuleRepository
import com.focusbridge.android.data.repository.ConfigRepository
import com.focusbridge.android.data.repository.NotificationRepository
import com.focusbridge.android.pairing.PhoneIdentity
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import kotlinx.serialization.json.JsonObject
import javax.inject.Inject
import javax.inject.Singleton

data class DesktopReconnectRequest(
    val deviceId: String?,
    val requestedAt: Long,
)

@Singleton
class WebSocketClient @Inject constructor(
    @ApplicationContext private val context: Context,
    private val okHttpClient: OkHttpClient,
    private val appInventoryProvider: AppInventoryProvider,
    private val appRules: AppRuleRepository,
    private val config: ConfigRepository,
    private val notifications: NotificationRepository,
    private val phoneIdentity: PhoneIdentity,
) {
    private var socket: WebSocket? = null
    @Volatile private var connectionSerial = 0
    @Volatile private var activePairingKey: String? = null
    @Volatile private var secureReady: Boolean = false
    @Volatile private var secureTransport: Boolean = false
    @Volatile private var lastPongAt: Long = 0L
    private var heartbeatJob: Job? = null
    private val _state = MutableStateFlow(ConnectionState.DISCONNECTED)
    private val _reconnectRequest = MutableStateFlow<DesktopReconnectRequest?>(null)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    val state: StateFlow<ConnectionState> = _state
    val reconnectRequest: StateFlow<DesktopReconnectRequest?> = _reconnectRequest

    fun isConnected(): Boolean = _state.value == ConnectionState.CONNECTED && socket != null

    fun connect(
        pairing: PairingEntity,
        deviceName: String = phoneIdentity.deviceName,
        endpointOverride: String? = null,
        retryingOnFailure: Boolean = false,
    ) {
        disconnect(showDisconnected = !retryingOnFailure)
        val serial = ++connectionSerial
        activePairingKey = pairing.pairingKey
        secureReady = false
        secureTransport = false
        _state.value = ConnectionState.CONNECTING
        val rawEndpoint = endpointOverride ?: pairing.endpoint
        val endpoint = if (rawEndpoint.startsWith("ws")) rawEndpoint else "ws://$rawEndpoint"
        val request = Request.Builder().url(endpoint).build()
        secureTransport = endpoint.startsWith("wss://") && pairing.certFingerprint.isNotBlank()
        val client = if (secureTransport) {
            okHttpClient.withPinnedCertificate(pairing.certFingerprint)
        } else {
            okHttpClient
        }
        socket = client.newWebSocket(
            request,
            object : WebSocketListener() {
                override fun onOpen(webSocket: WebSocket, response: Response) {
                    webSocket.send(
                        Protocol.auth(
                            pairingKey = pairing.pairingKey,
                            deviceId = pairing.deviceId,
                            deviceName = deviceName,
                            phoneInstallId = phoneIdentity.installId,
                        ),
                    )
                }

                override fun onMessage(webSocket: WebSocket, text: String) {
                    var envelope = runCatching { Protocol.decodeEnvelope(text) }.getOrNull() ?: return
                    if (envelope.type == MessageType.ENCRYPTED) {
                        val payload = envelope.payload as? JsonObject ?: return
                        val decrypted = runCatching { SecureEnvelope.decrypt(pairing.pairingKey, payload) }.getOrNull() ?: return
                        envelope = runCatching { Protocol.decodeEnvelope(decrypted) }.getOrNull() ?: return
                    }
                    when (envelope.type) {
                        MessageType.AUTH_OK -> {
                            secureReady = secureTransport
                            lastPongAt = System.currentTimeMillis()
                            updateState(serial, ConnectionState.CONNECTED)
                            startHeartbeat(webSocket, pairing.pairingKey, serial)
                            webSocket.sendSecure(pairing.pairingKey, Protocol.appInventory(appInventoryProvider.launchableApps()))
                        }
                        MessageType.AUTH_FAILED -> updateState(
                            serial,
                            if (retryingOnFailure) ConnectionState.RETRYING else ConnectionState.DISCONNECTED,
                        )
                        MessageType.PONG -> {
                            lastPongAt = System.currentTimeMillis()
                        }
                        MessageType.NOTIFICATION_ACK -> applyNotificationAck(envelope)
                        MessageType.RULES_UPDATE -> applyRulesUpdate(webSocket, envelope, pairing.pairingKey)
                        MessageType.DESKTOP_ACTION -> applyDesktopAction(envelope)
                        MessageType.UNPAIR -> applyManualDisconnect(webSocket, serial)
                        else -> Unit
                    }
                }

                override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                    stopHeartbeat(serial)
                    updateState(
                        serial,
                        if (retryingOnFailure) ConnectionState.RETRYING else ConnectionState.DISCONNECTED,
                    )
                }

                override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                    stopHeartbeat(serial)
                    updateState(
                        serial,
                        if (retryingOnFailure) ConnectionState.RETRYING else ConnectionState.DISCONNECTED,
                    )
                }
            },
        )
    }

    fun send(text: String): Boolean {
        val key = activePairingKey
        val body = if (secureReady && key != null) SecureEnvelope.encrypt(key, text) else text
        val accepted = socket?.send(body) == true
        if (!accepted && _state.value == ConnectionState.CONNECTED) {
            _state.value = ConnectionState.DISCONNECTED
        }
        return accepted
    }

    fun disconnect(showDisconnected: Boolean = true) {
        connectionSerial += 1
        socket?.close(1000, "FocusBridge disconnect")
        socket = null
        activePairingKey = null
        secureReady = false
        secureTransport = false
        stopHeartbeat()
        if (showDisconnected && _state.value != ConnectionState.DISCONNECTED) {
            _state.value = ConnectionState.DISCONNECTED
        }
    }

    fun manualDisconnect() {
        val message = Protocol.disconnectRequest()
        val key = activePairingKey
        if (key != null) {
            socket?.sendSecure(key, message)
        } else {
            socket?.send(message)
        }
        disconnect(showDisconnected = true)
        scope.launch {
            config.set("manual_disconnect", "true")
        }
    }

    fun acceptReconnectRequest() {
        scope.launch {
            config.set("manual_disconnect", "false")
            _reconnectRequest.value = null
        }
    }

    fun dismissReconnectRequest() {
        _reconnectRequest.value = null
    }

    private fun applyManualDisconnect(webSocket: WebSocket, serial: Int) {
        scope.launch {
            config.set("manual_disconnect", "true")
            webSocket.close(1000, "Desktop requested manual disconnect")
            if (serial == connectionSerial) {
                disconnect(showDisconnected = true)
            }
        }
    }

    private fun updateState(serial: Int, state: ConnectionState) {
        if (serial == connectionSerial) {
            _state.value = state
        }
    }

    private fun startHeartbeat(webSocket: WebSocket, pairingKey: String, serial: Int) {
        heartbeatJob?.cancel()
        heartbeatJob = scope.launch {
            while (serial == connectionSerial) {
                delay(HEARTBEAT_INTERVAL_MS)
                if (serial != connectionSerial || _state.value != ConnectionState.CONNECTED) {
                    continue
                }
                val age = System.currentTimeMillis() - lastPongAt
                if (age > HEARTBEAT_TIMEOUT_MS) {
                    webSocket.close(1001, "FocusBridge heartbeat timeout")
                    updateState(serial, ConnectionState.DISCONNECTED)
                    break
                }
                webSocket.sendSecure(pairingKey, Protocol.ping())
            }
        }
    }

    private fun stopHeartbeat(serial: Int? = null) {
        if (serial == null || serial == connectionSerial) {
            heartbeatJob?.cancel()
            heartbeatJob = null
        }
    }

    private fun applyRulesUpdate(webSocket: WebSocket, envelope: Envelope, pairingKey: String) {
        scope.launch {
            val update = Protocol.decodeRulesUpdate(envelope.payload)
            appRules.replaceFromDesktop(
                update.appRules.map { rule ->
                    AppRuleEntity(
                        packageName = rule.packageName,
                        muted = rule.muted,
                        priority = rule.priority,
                        studySafe = rule.studySafe,
                        updatedAt = System.currentTimeMillis(),
                    )
                },
            )
            config.set("priority_keywords", update.priorityKeywords.joinToString(","))
            config.set("blocked_keywords", update.blockedKeywords.joinToString(","))
            config.set("favorite_contacts", update.favoriteContacts.joinToString(","))
            webSocket.send(SecureEnvelope.encrypt(pairingKey, Protocol.rulesAck(update.appRules.size)))
        }
    }

    private fun applyNotificationAck(envelope: Envelope) {
        scope.launch {
            val ack = Protocol.decodeNotificationAck(envelope.payload)
            if (ack.accepted) {
                notifications.markSent(ack.id)
            }
        }
    }

    private fun applyDesktopAction(envelope: Envelope) {
        scope.launch {
            val action = Protocol.decodeDesktopAction(envelope.payload)
            if (action.action == "reconnect_request") {
                val request = DesktopReconnectRequest(action.deviceId, action.requestedAt)
                config.set("last_desktop_reconnect_request_at", action.requestedAt.toString())
                _reconnectRequest.value = request
                showReconnectNotification()
            }
        }
    }

    private fun showReconnectNotification() {
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(
                RECONNECT_CHANNEL_ID,
                "FocusBridge reconnect requests",
                NotificationManager.IMPORTANCE_HIGH,
            ),
        )
        val intent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP
            putExtra(MainActivity.EXTRA_SHOW_RECONNECT_PROMPT, true)
        }
        val pendingIntent = PendingIntent.getActivity(
            context,
            91,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val notification = NotificationCompat.Builder(context, RECONNECT_CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_launcher)
            .setContentTitle("Desktop wants to reconnect")
            .setContentText("Open FocusBridge and accept to resume sync.")
            .setContentIntent(pendingIntent)
            .setAutoCancel(true)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .build()
        manager.notify(RECONNECT_NOTIFICATION_ID, notification)
    }

    private fun WebSocket.sendSecure(pairingKey: String, message: String) {
        send(if (secureReady) SecureEnvelope.encrypt(pairingKey, message) else message)
    }

    private companion object {
        const val HEARTBEAT_INTERVAL_MS = 15_000L
        const val HEARTBEAT_TIMEOUT_MS = 180_000L
        const val RECONNECT_CHANNEL_ID = "focusbridge_reconnect"
        const val RECONNECT_NOTIFICATION_ID = 91
    }
}
