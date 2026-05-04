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
import kotlinx.serialization.json.JsonObject
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
    @Volatile private var activePairingKey: String? = null
    @Volatile private var secureReady: Boolean = false
    @Volatile private var secureTransport: Boolean = false
    private val _state = MutableStateFlow(ConnectionState.DISCONNECTED)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    val state: StateFlow<ConnectionState> = _state

    fun isConnected(): Boolean = _state.value == ConnectionState.CONNECTED && socket != null

    fun connect(
        pairing: PairingEntity,
        deviceName: String = "Android phone",
        endpointOverride: String? = null,
    ) {
        disconnect()
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
                    webSocket.send(Protocol.auth(pairing.pairingKey, pairing.deviceId, deviceName))
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
                            updateState(serial, ConnectionState.CONNECTED)
                            webSocket.sendSecure(pairing.pairingKey, Protocol.appInventory(appInventoryProvider.launchableApps()))
                        }
                        MessageType.AUTH_FAILED -> updateState(serial, ConnectionState.DISCONNECTED)
                        MessageType.RULES_UPDATE -> applyRulesUpdate(webSocket, envelope, pairing.pairingKey)
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

    fun send(text: String): Boolean {
        val key = activePairingKey
        val body = if (secureReady && key != null) SecureEnvelope.encrypt(key, text) else text
        return socket?.send(body) == true
    }

    fun disconnect() {
        connectionSerial += 1
        socket?.close(1000, "FocusBridge disconnect")
        socket = null
        activePairingKey = null
        secureReady = false
        secureTransport = false
        if (_state.value != ConnectionState.DISCONNECTED) {
            _state.value = ConnectionState.DISCONNECTED
        }
    }

    private fun updateState(serial: Int, state: ConnectionState) {
        if (serial == connectionSerial) {
            _state.value = state
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

    private fun WebSocket.sendSecure(pairingKey: String, message: String) {
        send(if (secureReady) SecureEnvelope.encrypt(pairingKey, message) else message)
    }
}
