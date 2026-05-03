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
) {
    private var socket: WebSocket? = null
    private val _state = MutableStateFlow(ConnectionState.DISCONNECTED)
    val state: StateFlow<ConnectionState> = _state

    fun connect(pairing: PairingEntity, deviceName: String = "Android phone") {
        _state.value = ConnectionState.CONNECTING
        val endpoint = if (pairing.endpoint.startsWith("ws")) pairing.endpoint else "ws://${pairing.endpoint}"
        val request = Request.Builder().url(endpoint).build()
        socket = okHttpClient.newWebSocket(
            request,
            object : WebSocketListener() {
                override fun onOpen(webSocket: WebSocket, response: Response) {
                    _state.value = ConnectionState.CONNECTED
                    webSocket.send(Protocol.auth(pairing.pairingKey, pairing.deviceId, deviceName))
                }

                override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                    _state.value = ConnectionState.DISCONNECTED
                }

                override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                    _state.value = ConnectionState.DISCONNECTED
                }
            },
        )
    }

    fun send(text: String): Boolean = socket?.send(text) == true

    fun disconnect() {
        socket?.close(1000, "FocusBridge disconnect")
        socket = null
        _state.value = ConnectionState.DISCONNECTED
    }
}
