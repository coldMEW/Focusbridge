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
import com.focusbridge.android.security.DeviceIdentityProvider
import com.focusbridge.android.sync.secure.PhoneSecureSession
import com.focusbridge.android.sync.secure.SecureStep
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import okio.ByteString.Companion.toByteString
import android.util.Base64
import kotlinx.serialization.json.JsonObject
import javax.inject.Inject
import javax.inject.Singleton

data class DesktopReconnectRequest(
    val deviceId: String?,
    val requestedAt: Long,
)

    /**
 * Unwraps the pairing-key envelope the desktop wraps outbound traffic in once
 * a socket is authenticated.
 *
 * Both transports need this. The desktop sends AUTH_OK in the clear and
 * encrypts everything after it, so a path that skipped this step would
 * authenticate correctly and then silently discard every reply - including
 * the PONG the phone's heartbeat waits for, which ends the session on a
 * timeout roughly three minutes later.
 */
internal fun unwrap(envelope: Envelope, pairingKey: String): Envelope? {
    if (envelope.type != MessageType.ENCRYPTED) return envelope
    val payload = envelope.payload as? JsonObject ?: return null
    val decrypted = runCatching { SecureEnvelope.decrypt(pairingKey, payload) }.getOrNull() ?: return null
    return runCatching { Protocol.decodeEnvelope(decrypted) }.getOrNull()
}


@Singleton
class WebSocketClient @Inject constructor(
    @ApplicationContext private val context: Context,
    private val okHttpClient: OkHttpClient,
    private val appInventoryProvider: AppInventoryProvider,
    private val appRules: AppRuleRepository,
    private val config: ConfigRepository,
    private val notifications: NotificationRepository,
    private val phoneIdentity: PhoneIdentity,
    private val deviceIdentity: DeviceIdentityProvider,
) {
    private var socket: WebSocket? = null
    @Volatile private var connectionSerial = 0
    @Volatile private var activePairingKey: String? = null
    @Volatile private var secureReady: Boolean = false
    @Volatile private var secureTransport: Boolean = false
    @Volatile private var lastPongAt: Long = 0L
    /** Non-null only on the relay transport, where it is the sole security boundary. */
    @Volatile private var secureSession: PhoneSecureSession? = null
    @Volatile private var relayTransport: Boolean = false
    /**
     * True while a relay socket is open, including while the desktop peer is absent.
     * The supervisor uses it to keep waiting on the relay instead of tearing the
     * socket down every retry tick, which would also lose the peer-arrival signal.
     */
    @Volatile private var relayAttached: Boolean = false
    /**
     * Set when the relay reports a desktop while this phone is configured to ask
     * first. Holding the peer here rather than refusing it is what lets a PC
     * reach this phone from any network without being able to connect silently.
     */
    private var pendingApproval: (() -> Unit)? = null
    private var heartbeatJob: Job? = null
    private var sessionJob = SupervisorJob()
    @Volatile private var manuallyDisconnected = false
    /**
     * Set when the user pairs this phone by scanning a code, and cleared once a
     * session is actually established.
     *
     * Scanning is consent. Without this, a phone with "let a known PC connect on
     * its own" switched off would ask for permission immediately after the user
     * had just granted it at the desktop -- and with no relay configured it would
     * not connect at all, because the switch also stops it dialing the address it
     * had just been given. It survives failed attempts on purpose: consent was
     * given for the connection, not for one particular try at it.
     */
    @Volatile private var pairedByUser = false
    /**
     * Set when the desktop says it does not recognise this pairing at all.
     *
     * Retrying cannot fix that: the credentials this phone holds are for a code
     * that has since been replaced, so every attempt fails identically, forever,
     * on mobile data. The phone stops dialing and says what to do instead, which
     * is the difference between an explanation and a spinner that never resolves.
     */
    private val _pairingRejection = MutableStateFlow<String?>(null)
    val pairingRejection: StateFlow<String?> = _pairingRejection
    private val preferenceMutex = Mutex()
    private val rulesUpdateMutex = Mutex()
    private val _state = MutableStateFlow(ConnectionState.DISCONNECTED)
    private val _reconnectRequest = MutableStateFlow<DesktopReconnectRequest?>(null)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    val state: StateFlow<ConnectionState> = _state
    val reconnectRequest: StateFlow<DesktopReconnectRequest?> = _reconnectRequest

    @Synchronized
    fun isConnected(): Boolean = _state.value == ConnectionState.CONNECTED && socket != null

    fun isManuallyDisconnected(): Boolean = manuallyDisconnected

    /** Called when the user pairs this phone by scanning a desktop code. */
    fun notePairedByUser() {
        pairedByUser = true
        _pairingRejection.value = null
    }

    /** Clears a terminal rejection so the phone will try again. */
    fun clearPairingRejection() {
        _pairingRejection.value = null
    }

    /** True until the connection the user asked for has been established. */
    fun hasPairingConsent(): Boolean = pairedByUser

    /**
     * True when a transport is established, even if the desktop has not yet been
     * authenticated. Never treat this as a connected phone in the UI.
     */
    @Synchronized
    fun isAwaitingPeer(): Boolean = relayAttached && socket != null && !isConnected()

    @Synchronized
    fun connect(
        pairing: PairingEntity,
        deviceName: String = phoneIdentity.deviceName,
        endpointOverride: String? = null,
        retryingOnFailure: Boolean = false,
        useRelay: Boolean = false,
        requireApproval: Boolean = false,
        awaitingRequest: Boolean = false,
    ) {
        // After a manual disconnect this phone stops dialing desktops, but it may
        // still wait to be asked for by name -- that is what makes reconnecting it
        // from another network possible. Waiting is the only thing allowed then,
        // and it is said outright rather than inferred from whether the user will
        // be prompted: the prompt is the switch's decision, and tying the two
        // together is what made a switched-on phone unable to reconnect at all.
        if (manuallyDisconnected && !awaitingRequest) return
        disconnect(showDisconnected = !retryingOnFailure)
        sessionJob = SupervisorJob()
        val serial = ++connectionSerial
        activePairingKey = pairing.pairingKey
        secureReady = false
        secureTransport = false
        relayTransport = useRelay
        _state.value = ConnectionState.CONNECTING

        val client: OkHttpClient
        val request: Request
        if (useRelay) {
            require(pairing.supportsRelay()) { "This pairing has no relay credentials; scan a new desktop QR" }
            // The relay is a public HTTPS origin, so it is validated against the
            // system trust store. Certificate pinning belongs to the LAN transport,
            // whose desktop certificate is self-signed and pinned from the QR.
            client = okHttpClient
            request = Request.Builder()
                .url(pairing.relaySocketUrl())
                .header("Authorization", "Bearer " + pairing.relayCapability)
                .build()
        } else {
            val rawEndpoint = endpointOverride ?: pairing.endpoint
            val endpoint = when {
                rawEndpoint.startsWith("wss://") -> rawEndpoint
                rawEndpoint.startsWith("ws://") -> "wss://" + rawEndpoint.removePrefix("ws://")
                else -> "wss://" + rawEndpoint
            }
            require(pairing.mode.equals("LOCAL", ignoreCase = true)) {
                "Relay sync requires the new end-to-end pairing protocol"
            }
            require(pairing.certFingerprint.matches(Regex("[a-fA-F0-9]{64}"))) {
                "Refresh the desktop QR and pair again: a valid certificate fingerprint is required"
            }
            secureTransport = true
            client = okHttpClient.withPinnedCertificate(pairing.certFingerprint)
            request = Request.Builder().url(endpoint).build()
        }

        socket = client.newWebSocket(
            request,
            object : WebSocketListener() {
                override fun onOpen(webSocket: WebSocket, response: Response): Unit = synchronized(this@WebSocketClient) {
                    if (serial != connectionSerial) { webSocket.cancel(); return@synchronized }
                    if (useRelay) {
                        // Wait for the relay to report the desktop peer. Starting a
                        // handshake into an absent peer would burn a session the
                        // relay cannot deliver.
                        relayAttached = true
                        return@synchronized
                    }
                    webSocket.send(
                        Protocol.auth(
                            pairingKey = pairing.pairingKey,
                            deviceId = pairing.deviceId,
                            deviceName = deviceName,
                            phoneInstallId = phoneIdentity.installId,
                        ),
                    )
                }

                override fun onMessage(webSocket: WebSocket, text: String): Unit = synchronized(this@WebSocketClient) {
                    if (serial != connectionSerial) return@synchronized
                    if (useRelay) {
                        // On the relay transport every inbound text frame is relay
                        // control metadata. Application data is always binary, so a
                        // text frame can never be mistaken for a peer message.
                        onRelayControl(webSocket, text, pairing, serial, retryingOnFailure, requireApproval)
                        return@synchronized
                    }
                    val decoded = runCatching { Protocol.decodeEnvelope(text) }.getOrNull() ?: return@synchronized
                    val envelope = unwrap(decoded, pairing.pairingKey) ?: return@synchronized
                    dispatch(webSocket, envelope, pairing, serial, retryingOnFailure)
                }

                override fun onMessage(webSocket: WebSocket, bytes: ByteString): Unit = synchronized(this@WebSocketClient) {
                    if (serial != connectionSerial || !useRelay) return@synchronized
                    onRelayFrame(webSocket, bytes, pairing, deviceName, serial, retryingOnFailure)
                }

                override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                    finishConnection(
                        serial,
                        if (retryingOnFailure) ConnectionState.RETRYING else ConnectionState.DISCONNECTED,
                    )
                }

                override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                    finishConnection(
                        serial,
                        if (retryingOnFailure) ConnectionState.RETRYING else ConnectionState.DISCONNECTED,
                    )
                }

                override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                    finishConnection(
                        serial,
                        if (retryingOnFailure) ConnectionState.RETRYING else ConnectionState.DISCONNECTED,
                    )
                }
            },
        )
    }

    /**
     * Handles one relay control frame. `relay.peer_ready` carries a generation that
     * changes whenever either endpoint reconnects, so it always starts a brand new
     * Noise session rather than reusing keys across peers.
     */
    private fun onRelayControl(
        webSocket: WebSocket,
        text: String,
        pairing: PairingEntity,
        serial: Int,
        retryingOnFailure: Boolean,
        requireApproval: Boolean,
    ) {
        when (runCatching { Protocol.relayControlType(text) }.getOrNull()) {
            "relay.peer_ready" -> {
                if (requireApproval) {
                    // Remember how to continue, then ask. Nothing is decrypted and
                    // no notification leaves this phone until the user accepts.
                    val alreadyAsking = _reconnectRequest.value != null
                    pendingApproval = { startSecureSession(webSocket, pairing, serial, retryingOnFailure) }
                    _reconnectRequest.value = DesktopReconnectRequest(pairing.deviceId, System.currentTimeMillis())
                    // Notifying on every relay event is how one desktop becomes a
                    // stream of identical notifications.
                    if (!alreadyAsking) showReconnectNotification()
                } else {
                    startSecureSession(webSocket, pairing, serial, retryingOnFailure)
                }
            }
            "relay.peer_unavailable" -> {
                // Keep the relay socket so the desktop's arrival still reaches us,
                // but stop advertising a peer that is not there.
                pendingApproval = null
                closeSecureSession()
                secureReady = false
                if (serial == connectionSerial && _state.value == ConnectionState.CONNECTED) {
                    _state.value = if (retryingOnFailure) ConnectionState.RETRYING else ConnectionState.DISCONNECTED
                }
            }
            else -> Unit
        }
    }

    private fun startSecureSession(
        webSocket: WebSocket,
        pairing: PairingEntity,
        serial: Int,
        retryingOnFailure: Boolean,
    ) {
        closeSecureSession()
        secureReady = false
        var privateKey: ByteArray? = null
        var psk: ByteArray? = null
        try {
            privateKey = deviceIdentity.privateKey()
            psk = decodeKey(pairing.enrollmentPsk, PhoneSecureSession.KEY_BYTES, "enrollment key")
            val desktopKey = decodeKey(pairing.desktopPublicKey, PhoneSecureSession.KEY_BYTES, "desktop key")
            val pairId = decodeHex(pairing.relayPairId, PhoneSecureSession.PAIR_ID_BYTES)
            val session = PhoneSecureSession.create()
            // start() consumes and wipes both secrets on every outcome.
            val first = session.start(privateKey, psk, pairId, desktopKey)
            privateKey = null
            psk = null
            secureSession = session
            webSocket.send(first.toByteString())
        } catch (failure: Throwable) {
            android.util.Log.w("FocusBridgeSync", "Secure session could not start", failure)
            closeSecureSession()
            finishConnection(serial, if (retryingOnFailure) ConnectionState.RETRYING else ConnectionState.DISCONNECTED)
        } finally {
            // Native code wipes these itself; this covers a library-load or
            // invocation failure that never reached native code.
            privateKey?.fill(0)
            psk?.fill(0)
        }
    }

    private fun onRelayFrame(
        webSocket: WebSocket,
        bytes: ByteString,
        pairing: PairingEntity,
        deviceName: String,
        serial: Int,
        retryingOnFailure: Boolean,
    ) {
        val session = secureSession ?: return
        val step = try {
            session.receive(bytes.toByteArray())
        } catch (failure: Throwable) {
            // A Noise session that has seen a bad frame can no longer tell replay
            // from ordinary loss, so it is discarded rather than retried.
            android.util.Log.w("FocusBridgeSync", "Secure session failed; reconnecting", failure)
            closeSecureSession()
            finishConnection(serial, if (retryingOnFailure) ConnectionState.RETRYING else ConnectionState.DISCONNECTED)
            return
        }
        when (step) {
            is SecureStep.Send -> {
                step.frames.forEach { webSocket.send(it.toByteString()) }
                if (session.isReady) {
                    // Only now is the desktop authenticated as the pinned device.
                    secureReady = true
                    sendEnvelope(
                        webSocket,
                        pairing.pairingKey,
                        Protocol.auth(
                            pairingKey = pairing.pairingKey,
                            deviceId = pairing.deviceId,
                            deviceName = deviceName,
                            phoneInstallId = phoneIdentity.installId,
                        ),
                    )
                }
            }
            is SecureStep.Deliver -> {
                val plaintext = step.plaintext
                val envelope = try {
                    runCatching { Protocol.decodeEnvelope(String(plaintext, Charsets.UTF_8)) }.getOrNull()
                } finally {
                    plaintext.fill(0)
                }
                val unwrapped = envelope?.let { unwrap(it, pairing.pairingKey) }
                if (unwrapped != null) dispatch(webSocket, unwrapped, pairing, serial, retryingOnFailure)
            }
            SecureStep.Continue -> Unit
        }
    }

    private fun dispatch(
        webSocket: WebSocket,
        envelope: Envelope,
        pairing: PairingEntity,
        serial: Int,
        retryingOnFailure: Boolean,
    ) {
        when (envelope.type) {
            MessageType.AUTH_OK -> {
                if (!relayTransport) secureReady = secureTransport
                lastPongAt = System.currentTimeMillis()
                // The consent given by scanning has been used now.
                pairedByUser = false
                // An authenticated session ends the disconnect, whether the user
                // accepted a request or the switch let this through silently.
                // Holding both states at once is not a subtlety: the phone shows
                // connected while every notification is dropped before it is sent,
                // because the send path checks the disconnect first.
                if (manuallyDisconnected) setManualDisconnect(false)
                updateState(serial, ConnectionState.CONNECTED)
                startHeartbeat(webSocket, pairing.pairingKey, serial)
                sendAppInventory(webSocket, pairing.pairingKey, serial)
            }
            MessageType.AUTH_FAILED -> {
                val reason = (envelope.payload as? JsonObject)
                    ?.get("reason")?.toString()?.trim('"')
                if (reason == "unknown_pairing") {
                    // Terminal: only pairing again can resolve it.
                    _pairingRejection.value =
                        "This PC no longer recognises this pairing. Scan the code on the PC again."
                    finishConnection(serial, ConnectionState.DISCONNECTED)
                } else {
                    finishConnection(
                        serial,
                        if (retryingOnFailure) ConnectionState.RETRYING else ConnectionState.DISCONNECTED,
                    )
                }
            }
            MessageType.PONG -> {
                lastPongAt = System.currentTimeMillis()
            }
            MessageType.NOTIFICATION_ACK -> applyNotificationAck(envelope)
            MessageType.RULES_UPDATE -> applyRulesUpdate(webSocket, envelope, pairing.pairingKey)
            MessageType.DESKTOP_ACTION -> applyDesktopAction(envelope)
            MessageType.UNPAIR -> applyManualDisconnect(serial)
            else -> Unit
        }
    }

    private fun closeSecureSession() {
        secureSession?.close()
        secureSession = null
    }

    private fun decodeKey(value: String, size: Int, label: String): ByteArray {
        val decoded = try {
            Base64.decode(value, Base64.NO_WRAP)
        } catch (failure: IllegalArgumentException) {
            throw IllegalArgumentException("Invalid " + label + " in this pairing", failure)
        }
        if (decoded.size != size) {
            decoded.fill(0)
            throw IllegalArgumentException("Invalid " + label + " length in this pairing")
        }
        return decoded
    }

    private fun decodeHex(value: String, size: Int): ByteArray {
        require(value.length == size * 2 && value.all { it in "0123456789abcdef" }) {
            "Invalid relay pair identifier in this pairing"
        }
        return ByteArray(size) { value.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
    }

    @Synchronized
    fun send(text: String): Boolean {
        if (!isConnected()) return false
        val key = activePairingKey ?: return false
        val webSocket = socket ?: return false
        val accepted = sendEnvelope(webSocket, key, text)
        if (!accepted && _state.value == ConnectionState.CONNECTED) {
            disconnect()
        }
        return accepted
    }

    /**
     * Sends one plaintext FocusBridge envelope over whichever transport is active.
     *
     * On the relay the record is sealed into ordered Noise frames; a partial write
     * cannot be retried, so a refused frame fails the whole send and the session is
     * discarded by the caller. On the LAN it keeps the pairing-key envelope format.
     */
    private fun sendEnvelope(webSocket: WebSocket, pairingKey: String, text: String): Boolean {
        if (!relayTransport) {
            return webSocket.send(if (secureReady) SecureEnvelope.encrypt(pairingKey, text) else text)
        }
        val session = secureSession ?: return false
        val frames = try {
            session.seal(text.toByteArray(Charsets.UTF_8))
        } catch (failure: Throwable) {
            android.util.Log.w("FocusBridgeSync", "Secure send failed; session discarded", failure)
            closeSecureSession()
            return false
        }
        for (frame in frames) {
            if (!webSocket.send(frame.toByteString())) {
                closeSecureSession()
                return false
            }
        }
        return true
    }

    @Synchronized
    fun disconnect(showDisconnected: Boolean = true) {
        connectionSerial += 1
        sessionJob.cancel()
        socket?.close(1000, "FocusBridge disconnect")
        socket = null
        activePairingKey = null
        secureReady = false
        secureTransport = false
        relayTransport = false
        relayAttached = false
        pendingApproval = null
        closeSecureSession()
        stopHeartbeat()
        if (showDisconnected && _state.value != ConnectionState.DISCONNECTED) {
            _state.value = ConnectionState.DISCONNECTED
        }
    }

    @Synchronized
    fun manualDisconnect() {
        setManualDisconnect(true)
        val message = Protocol.disconnectRequest()
        val key = activePairingKey
        val webSocket = socket
        if (webSocket != null) {
            if (key != null) sendEnvelope(webSocket, key, message) else webSocket.send(message)
        }
        disconnect(showDisconnected = true)
    }

    @Synchronized
    fun acceptReconnectRequest() {
        _pairingRejection.value = null
        setManualDisconnect(false)
        _reconnectRequest.value = null
        // A desktop waiting at the relay can be answered immediately; there is no
        // need to wait for the next supervisor tick.
        pendingApproval?.let { start ->
            pendingApproval = null
            start()
        }
    }

    private fun setManualDisconnect(value: Boolean) {
        manuallyDisconnected = value
        // Enqueue writes in intent order, even if an earlier database write suspends.
        scope.launch(start = CoroutineStart.UNDISPATCHED) {
            preferenceMutex.withLock { config.set("manual_disconnect", value.toString()) }
        }
    }

    fun dismissReconnectRequest() {
        _reconnectRequest.value = null
    }

    private fun applyManualDisconnect(serial: Int) {
        if (serial == connectionSerial) {
            setManualDisconnect(true)
            disconnect(showDisconnected = true)
        }
    }

    @Synchronized
    private fun finishConnection(serial: Int, state: ConnectionState) {
        if (serial != connectionSerial) return
        disconnect(showDisconnected = false)
        _state.value = state
    }

    private fun sendAppInventory(webSocket: WebSocket, pairingKey: String, serial: Int) {
        scope.launch(sessionJob) {
            val apps = try {
                appInventoryProvider.launchableApps()
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (failure: Exception) {
                android.util.Log.w("FocusBridgeSync", "App inventory unavailable; keeping desktop inventory", failure)
                return@launch
            }
            synchronized(this@WebSocketClient) {
                if (serial == connectionSerial && isConnected()) {
                    sendEnvelope(webSocket, pairingKey, Protocol.appInventory(apps))
                }
            }
        }
    }

    @Synchronized
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
                synchronized(this@WebSocketClient) {
                    if (serial != connectionSerial || _state.value != ConnectionState.CONNECTED) {
                        return@launch
                    }
                    val age = System.currentTimeMillis() - lastPongAt
                    if (age > HEARTBEAT_TIMEOUT_MS) {
                        finishConnection(serial, ConnectionState.DISCONNECTED)
                        return@launch
                    }
                    sendEnvelope(webSocket, pairingKey, Protocol.ping())
                }
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
        // Enter the mutex queue in receive order, not IO dispatcher scheduling order.
        scope.launch(sessionJob, start = CoroutineStart.UNDISPATCHED) {
            rulesUpdateMutex.withLock {
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
                synchronized(this@WebSocketClient) {
                    sendEnvelope(webSocket, pairingKey, Protocol.rulesAck(update.appRules.size))
                }
            }
        }
    }

    private fun applyNotificationAck(envelope: Envelope) {
        scope.launch(sessionJob) {
            val ack = Protocol.decodeNotificationAck(envelope.payload)
            if (ack.accepted) {
                notifications.markSent(ack.id)
            }
        }
    }

    private fun applyDesktopAction(envelope: Envelope) {
        scope.launch(sessionJob) {
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

    private companion object {
        const val HEARTBEAT_INTERVAL_MS = 15_000L
        const val HEARTBEAT_TIMEOUT_MS = 180_000L
        const val RECONNECT_CHANNEL_ID = "focusbridge_reconnect"
        const val RECONNECT_NOTIFICATION_ID = 91
    }
}
