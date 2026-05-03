package com.focusbridge.android.sync

import com.focusbridge.android.data.local.PairingEntity
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
) {
    private var socket: WebSocket? = null
    @Volatile private var connectionSerial = 0
    private val _state = MutableStateFlow(ConnectionState.DISCONNECTED)
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
                    when {
                        text.contains("\"AUTH_OK\"") -> {
                            updateState(serial, ConnectionState.CONNECTED)
                            webSocket.send(Protocol.appInventory(appInventoryProvider.launchableApps()))
                        }
                        text.contains("\"AUTH_FAILED\"") -> updateState(serial, ConnectionState.DISCONNECTED)
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
}
