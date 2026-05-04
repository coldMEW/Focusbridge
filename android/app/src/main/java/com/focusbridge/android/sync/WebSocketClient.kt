package com.focusbridge.android.sync

import com.focusbridge.android.data.local.AppRuleEntity
import com.focusbridge.android.data.local.PairingEntity
import com.focusbridge.android.data.repository.AppRuleRepository
import com.focusbridge.android.data.repository.ConfigRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class WebSocketClient @Inject constructor(
    private val okHttpClient: OkHttpClient,
    private val appInventoryProvider: AppInventoryProvider,
    private val appRules: AppRuleRepository,
    private val config: ConfigRepository,
) {
    private var socket: WebSocket? = null
    @Volatile private var connectionSerial = 0
    private val _state = MutableStateFlow(ConnectionState.DISCONNECTED)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    val state: StateFlow<ConnectionState> = _state

    fun connect(
        pairing: PairingEntity,
        deviceName: String = "Android phone",
        endpointOverride: String? = null,
    ) {
        disconnect()
        val serial = ++connectionSerial
        _state.value = ConnectionState.CONNECTING
        val rawEndpoint = endpointOverride ?: pairing.endpoint
        val endpoint = if (rawEndpoint.startsWith("ws")) rawEndpoint else "ws://$rawEndpoint"
        val request = Request.Builder().url(endpoint).build()
        socket = okHttpClient.newWebSocket(
            request,
            object : WebSocketListener() {
                override fun onOpen(webSocket: WebSocket, response: Response) {
                    webSocket.send(Protocol.auth(pairing.pairingKey, pairing.deviceId, deviceName))
                }

                override fun onMessage(webSocket: WebSocket, text: String) {
                    val envelope = runCatching { Protocol.decodeEnvelope(text) }.getOrNull() ?: return
                    when (envelope.type) {
                        MessageType.AUTH_OK -> {
                            updateState(serial, ConnectionState.CONNECTED)
                            webSocket.send(Protocol.appInventory(appInventoryProvider.launchableApps()))
                        }
                        MessageType.AUTH_FAILED -> updateState(serial, ConnectionState.DISCONNECTED)
                        MessageType.RULES_UPDATE -> applyRulesUpdate(webSocket, envelope)
                        else -> Unit
                    }
                }

                override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                    updateState(serial, ConnectionState.DISCONNECTED)
                }

                override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                    updateState(serial, ConnectionState.DISCONNECTED)
                }
            },
        )
    }

    fun send(text: String): Boolean = socket?.send(text) == true

    fun disconnect() {
        connectionSerial += 1
        socket?.close(1000, "FocusBridge disconnect")
        socket = null
        if (_state.value != ConnectionState.DISCONNECTED) {
            _state.value = ConnectionState.DISCONNECTED
        }
    }

    private fun updateState(serial: Int, state: ConnectionState) {
        if (serial == connectionSerial) {
            _state.value = state
        }
    }

    private fun applyRulesUpdate(webSocket: WebSocket, envelope: Envelope) {
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
            webSocket.send(Protocol.rulesAck(update.appRules.size))
        }
    }
}
