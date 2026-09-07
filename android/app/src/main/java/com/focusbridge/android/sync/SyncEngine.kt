package com.focusbridge.android.sync

import com.focusbridge.android.data.local.NotificationEntity
import com.focusbridge.android.data.repository.NotificationRepository
import com.focusbridge.android.data.repository.PairingRepository
import com.focusbridge.android.data.repository.ConfigRepository
import com.focusbridge.android.pairing.DeviceInfo
import android.util.Log
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.delay
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.isActive
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeoutOrNull

@Singleton
class SyncEngine @Inject constructor(
    private val pairings: PairingRepository,
    private val notifications: NotificationRepository,
    private val client: WebSocketClient,
    private val config: ConfigRepository,
) {
    private val connectMutex = Mutex()

    suspend fun maintainActivePairing() {
        while (currentCoroutineContext().isActive) {
            try {
                // Disconnect means disconnect: this phone does not dial a desktop
                // after the user has let one go. It does stay findable, which is
                // what makes "reconnect this phone" work from another network --
                // and being asked for by name is a deliberate act at the other
                // end, not this phone undoing the user's decision. Whether that
                // request prompts is the switch's business, not the disconnect's.
                if (client.pairingRejection.value != null) {
                    // Nothing to attempt: the desktop does not know this pairing,
                    // and trying again produces the same answer every fifteen
                    // seconds for as long as the phone is switched on. The user
                    // has been told to scan again; wait for them to do it.
                } else if (isManuallyDisconnected()) {
                    if (!client.isAwaitingPeer()) awaitApproval()
                } else if (client.isConnected()) {
                    flushPending()
                } else if (!client.isAwaitingPeer()) {
                    connectActivePairing()
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (failure: Exception) {
                android.util.Log.w("FocusBridgeSync", "Sync attempt failed; retry scheduled", failure)
                client.disconnect(showDisconnected = true)
            }
            delay(RECONNECT_INTERVAL_MS)
        }
    }

    /**
     * Waits at the relay to be asked for, without dialing anyone.
     *
     * Whether being asked then prompts is the switch's decision, and only the
     * switch's: on means a PC that has connected before reconnects with no
     * notification, which is what the setting says on the tin. That does not
     * reopen the disconnect, because nothing here reaches for a desktop -- it
     * waits to be asked for by name, and a paused desktop never asks.
     *
     * A pairing with no relay stays entirely offline, having no way to be asked.
     */
    private suspend fun awaitApproval() {
        connectMutex.withLock {
            if (client.isConnected() || client.isAwaitingPeer()) return@withLock
            val pairing = pairings.active() ?: return@withLock
            if (!pairing.supportsRelay()) return@withLock
            client.connect(
                pairing,
                deviceName = DeviceInfo.deviceName,
                retryingOnFailure = true,
                useRelay = true,
                requireApproval = !autoReconnectEnabled(),
                awaitingRequest = true,
            )
        }
    }

    suspend fun connectActivePairing() {
        connectActivePairing(flushAfterConnect = true)
    }

    private suspend fun connectActivePairing(flushAfterConnect: Boolean) {
        var connectedNow = false
        connectMutex.withLock {
            if (client.isConnected()) {
                connectedNow = true
                return@withLock
            }
            val pairing = pairings.active() ?: return@withLock
            // Consent given by scanning counts as permission for this connection,
            // whatever the switch says. Without it, pairing with the switch off
            // asks the user to approve the pairing they just made -- and with no
            // relay configured it never connects at all, because the switch also
            // stops this phone dialing the address it was just handed.
            val automatic = autoReconnectEnabled() || client.hasPairingConsent()
            val quiet = isManuallyDisconnected()

            // The local path is preferred: no account, no relay hop, and it keeps
            // working with no Internet at all. It is only usable when this phone
            // may connect without asking, because dialing an address is the only
            // way to reach it and there is no moment at which to prompt.
            if (automatic && !quiet) {
                for (endpoint in pairing.candidateEndpoints()) {
                    if (isManuallyDisconnected()) return@withLock
                    client.connect(
                        pairing,
                        deviceName = DeviceInfo.deviceName,
                        endpointOverride = endpoint,
                        retryingOnFailure = true,
                    )
                    if (awaitConnected(CONNECT_TIMEOUT_MS)) {
                        connectedNow = true
                        return@withLock
                    }
                }
            }

            // The relay crosses networks, and is also the only way a PC can ask
            // for this phone at all, so it is joined even while disconnected. It
            // asks first exactly when automatic reconnection is off.
            if (pairing.supportsRelay()) {
                client.connect(
                    pairing,
                    deviceName = DeviceInfo.deviceName,
                    retryingOnFailure = true,
                    useRelay = true,
                    requireApproval = !automatic,
                )
                if (awaitConnected(RELAY_CONNECT_TIMEOUT_MS)) {
                    connectedNow = true
                    return@withLock
                }
                // Hold an attached relay socket open even though the desktop has
                // not answered: the relay reports its arrival on this same socket.
                if (client.isAwaitingPeer()) return@withLock
            }
            client.disconnect(showDisconnected = true)
        }
        if (connectedNow && flushAfterConnect) {
            flushPending()
        }
    }

    suspend fun send(notification: NotificationEntity) {
        // A live session outranks a stale disconnect. Checking the flag first meant
        // that a phone which had once been disconnected, and had since reconnected,
        // dropped every notification here while showing itself as connected and
        // synced -- the worst kind of failure, because nothing looks wrong.
        if (!client.isConnected() && isManuallyDisconnected()) {
            Log.i(TAG, "holding a notification: this phone is disconnected")
            return
        }
        if (!client.isConnected()) {
            connectActivePairing(flushAfterConnect = false)
        }
        if (client.send(Protocol.notification(notification))) return
        // One retry on a fresh connection; the row stays pending either way and
        // the supervisor flushes it when a session comes back.
        connectActivePairing(flushAfterConnect = false)
        if (!client.send(Protocol.notification(notification))) {
            Log.w(TAG, "could not send a notification; it stays pending")
        }
    }

    suspend fun flushPending() {
        notifications.pending().forEach { send(it) }
    }

    /**
     * Waits for this connection attempt to settle. Only CONNECTED counts: the relay
     * transport also has to complete a device-only handshake and an authenticated
     * exchange before the desktop is really reachable.
     */
    private suspend fun awaitConnected(timeoutMs: Long): Boolean =
        withTimeoutOrNull(timeoutMs) {
            client.state.first {
                it == ConnectionState.CONNECTED ||
                    it == ConnectionState.RETRYING ||
                    it == ConnectionState.DISCONNECTED
            }
        } == ConnectionState.CONNECTED

    /**
     * Whether the background supervisor may dial the saved desktop on its own.
     *
     * Off, this phone still connects when the user asks, but it will not attach
     * itself to the last desktop it saw. That matters when more than one PC has
     * been paired: silently reattaching sends notifications to whichever machine
     * answers first, which may not be the one the user is sitting at.
     *
     * Defaults to on, so an upgrade does not quietly stop syncing.
     */
    suspend fun autoReconnectEnabled(): Boolean = config.get(AUTO_RECONNECT_KEY) != "false"

    suspend fun setAutoReconnect(enabled: Boolean) {
        config.set(AUTO_RECONNECT_KEY, enabled.toString())
    }

    private suspend fun isManuallyDisconnected(): Boolean =
        client.isManuallyDisconnected() || config.get("manual_disconnect") == "true"

    companion object {
        private const val TAG = "FocusBridgeSync"
        const val AUTO_RECONNECT_KEY = "auto_reconnect"
        const val CONNECT_TIMEOUT_MS = 4_000L
        // The relay adds a round trip to Cloudflare plus a full Noise handshake and
        // an authenticated exchange, so it needs a longer budget than a LAN dial.
        const val RELAY_CONNECT_TIMEOUT_MS = 12_000L
        const val RECONNECT_INTERVAL_MS = 15_000L
    }
}
