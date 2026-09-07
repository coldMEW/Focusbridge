package com.focusbridge.android.sync

import com.focusbridge.android.data.local.NotificationEntity
import com.focusbridge.android.data.local.PairingEntity
import com.focusbridge.android.data.repository.ConfigRepository
import com.focusbridge.android.data.repository.NotificationRepository
import com.focusbridge.android.data.repository.PairingRepository
import com.focusbridge.android.pairing.DeviceInfo
import io.mockk.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test

class SyncEngineTest {
    private val pairings = mockk<PairingRepository>()
    private val notifications = mockk<NotificationRepository>(relaxed = true)
    private val config = mockk<ConfigRepository>(relaxed = true)
    private val client = mockk<WebSocketClient>(relaxed = true)
    private val state = MutableStateFlow(ConnectionState.DISCONNECTED)
    private val pairing = PairingEntity(
        "desktop", "wss://first", endpointCandidates = "wss://second",
        pairingKey = "a".repeat(64), certFingerprint = "b".repeat(64),
    )
    private val engine = SyncEngine(pairings, notifications, client, config)
    private val attempts = mutableListOf<String>()
    private val relayPairing = pairing.copy(
        relayUrl = "https://relay.example",
        relayAccountKey = "a".repeat(64),
        relayPairId = "b".repeat(32),
        relayCapability = "C".repeat(43),
        desktopPublicKey = "AAAA",
        enrollmentPsk = "BBBB",
    )

    @Before fun setup() {
        mockkObject(DeviceInfo)
        every { DeviceInfo.deviceName } returns "Phone"
        coEvery { pairings.active() } returns pairing
        every { client.state } returns state
        every { client.isConnected() } answers { state.value == ConnectionState.CONNECTED }
        every { client.hasPairingConsent() } returns false
        every { client.pairingRejection } returns MutableStateFlow(null)
        every { client.connect(any(), any(), any(), any()) } answers {
            attempts += thirdArg<String>()
            state.value = ConnectionState.CONNECTING
        }
    }

    @After fun cleanup() { unmockkObject(DeviceInfo) }

    @Test fun immediateFailureFallsBackWithoutWaitingForTimeout() = runBlocking {
        every { client.connect(any(), any(), any(), any()) } answers {
            attempts += thirdArg<String>()
            state.value = if (attempts.size == 1) ConnectionState.RETRYING else ConnectionState.CONNECTED
        }
        withTimeout(1_000) { engine.connectActivePairing() }
        assertEquals(listOf("wss://first", "wss://second"), attempts)
        coVerify(exactly = 1) { notifications.pending() }
    }

    @Test fun asynchronousFailureFallsBackWithoutWaitingForTimeout() = runBlocking {
        val job = launch(start = CoroutineStart.UNDISPATCHED) { engine.connectActivePairing() }
        state.value = ConnectionState.RETRYING
        yield()
        assertEquals(listOf("wss://first", "wss://second"), attempts)
        state.value = ConnectionState.CONNECTED
        withTimeout(1_000) { job.join() }
    }

    @Test fun disconnectedAttemptFallsBackWithoutWaitingForTimeout() = runBlocking {
        val job = launch(start = CoroutineStart.UNDISPATCHED) { engine.connectActivePairing() }
        state.value = ConnectionState.DISCONNECTED
        yield()
        assertEquals(2, attempts.size)
        job.cancelAndJoin()
    }

    @Test fun manualDisconnectDuringConnectStopsFallbackPromptly() = runBlocking {
        val job = launch(start = CoroutineStart.UNDISPATCHED) { engine.connectActivePairing() }
        every { client.isManuallyDisconnected() } returns true
        state.value = ConnectionState.DISCONNECTED
        withTimeout(1_000) { job.join() }
        assertEquals(listOf("wss://first"), attempts)
        coVerify(exactly = 0) { notifications.pending() }
    }

    @Test fun connectingAttemptKeepsTimeoutBeforeFallback() = runBlocking {
        val fallback = CompletableDeferred<Unit>()
        every { client.connect(any(), any(), any(), any()) } answers {
            attempts += thirdArg<String>()
            state.value = ConnectionState.CONNECTING
            if (attempts.size == 2) fallback.complete(Unit)
        }
        val started = System.nanoTime()
        val job = launch(start = CoroutineStart.UNDISPATCHED) { engine.connectActivePairing() }
        try {
            withTimeout(6_000) { fallback.await() }
            assertTrue((System.nanoTime() - started) / 1_000_000 >= 4_000)
            assertEquals(2, attempts.size)
        } finally {
            job.cancelAndJoin()
        }
    }

    @Test fun persistedManualDisconnectPreventsAttempts() = runBlocking {
        coEvery { config.get("manual_disconnect") } returns "true"
        engine.connectActivePairing()
        assertTrue(attempts.isEmpty())
    }

    @Test fun cancellationStopsFallbackAndReleasesMutex() = runBlocking {
        val job = launch(start = CoroutineStart.UNDISPATCHED) { engine.connectActivePairing() }
        job.cancelAndJoin()
        state.value = ConnectionState.RETRYING
        yield()
        assertTrue(job.isCancelled)
        assertEquals(listOf("wss://first"), attempts)
        every { client.connect(any(), any(), any(), any()) } answers {
            state.value = ConnectionState.CONNECTED
        }
        withTimeout(1_000) { engine.connectActivePairing() }
    }

    @Test fun failedEndpointRoundStillWaitsBeforeRetrying() = runBlocking {
        every { client.connect(any(), any(), any(), any()) } answers {
            attempts += thirdArg<String>()
            state.value = ConnectionState.RETRYING
        }
        val job = launch(start = CoroutineStart.UNDISPATCHED) { engine.maintainActivePairing() }
        try {
            assertEquals(2, attempts.size)
            delay(200)
            assertEquals(2, attempts.size)
            verify(exactly = 1) { client.disconnect(showDisconnected = true) }
        } finally {
            job.cancelAndJoin()
        }
    }

    @Test fun autoReconnectOffNeverDialsALocalAddress() = runBlocking {
        coEvery { config.get(SyncEngine.AUTO_RECONNECT_KEY) } returns "false"
        val job = launch(start = CoroutineStart.UNDISPATCHED) { engine.maintainActivePairing() }
        try {
            // A local address can only be reached by dialing it, so there is no way
            // to ask the user first. That path stays off entirely.
            assertTrue(attempts.isEmpty())
            delay(200)
            assertTrue(attempts.isEmpty())
        } finally {
            job.cancelAndJoin()
        }
    }

    /** Every attempt fails at once, so the engine reaches the relay leg. */
    private fun failEveryAttempt() {
        every { client.connect(any(), any(), any(), any(), any(), any()) } answers {
            state.value = ConnectionState.RETRYING
        }
    }

    @Test fun autoReconnectOffStillJoinsTheRelayButAsksBeforeLettingAPcIn() = runBlocking {
        coEvery { config.get(SyncEngine.AUTO_RECONNECT_KEY) } returns "false"
        coEvery { pairings.active() } returns relayPairing
        failEveryAttempt()

        withTimeout(2_000) { engine.connectActivePairing() }

        // Staying reachable is the point: a PC on another network cannot dial this
        // phone, so the phone has to be waiting at the relay in order to be asked.
        verify(exactly = 1) {
            client.connect(relayPairing, any(), any(), any(), useRelay = true, requireApproval = true)
        }
        // And it must not have dialed a local address, which cannot ask first.
        verify(exactly = 0) {
            client.connect(any(), any(), any(), any(), useRelay = false, requireApproval = any())
        }
    }

    @Test fun autoReconnectOnDialsLocallyThenJoinsTheRelayWithoutAsking() = runBlocking {
        coEvery { config.get(SyncEngine.AUTO_RECONNECT_KEY) } returns "true"
        coEvery { pairings.active() } returns relayPairing
        failEveryAttempt()

        withTimeout(2_000) { engine.connectActivePairing() }

        verify(exactly = 2) {
            client.connect(any(), any(), any(), any(), useRelay = false, requireApproval = false)
        }
        verify(exactly = 1) {
            client.connect(relayPairing, any(), any(), any(), useRelay = true, requireApproval = false)
        }
    }

    @Test fun autoReconnectDefaultsToOnSoUpgradesKeepSyncing() = runBlocking {
        coEvery { config.get(SyncEngine.AUTO_RECONNECT_KEY) } returns null
        assertTrue(engine.autoReconnectEnabled())
        coEvery { config.get(SyncEngine.AUTO_RECONNECT_KEY) } returns "true"
        assertTrue(engine.autoReconnectEnabled())
        coEvery { config.get(SyncEngine.AUTO_RECONNECT_KEY) } returns "false"
        assertFalse(engine.autoReconnectEnabled())
    }

    @Test fun theSupervisorStillDialsWhenAutoReconnectIsOn() = runBlocking {
        coEvery { config.get(SyncEngine.AUTO_RECONNECT_KEY) } returns "true"
        val job = launch(start = CoroutineStart.UNDISPATCHED) { engine.maintainActivePairing() }
        try {
            assertTrue(attempts.isNotEmpty())
        } finally {
            job.cancelAndJoin()
        }
    }

    @Test fun aDisconnectedPhoneWithNoRelayStaysOffTheNetwork() = runBlocking {
        coEvery { config.get(SyncEngine.AUTO_RECONNECT_KEY) } returns "false"
        coEvery { config.get("manual_disconnect") } returns "true"
        coEvery { pairings.active() } returns pairing
        failEveryAttempt()

        val job = launch(start = CoroutineStart.UNDISPATCHED) { engine.maintainActivePairing() }
        try {
            // With no relay there is no way to be asked, so a disconnected
            // LAN-only pairing stays entirely offline.
            verify(exactly = 0) { client.connect(any(), any(), any(), any(), any(), any()) }
        } finally {
            job.cancelAndJoin()
        }
    }

    private val sampleNotification = NotificationEntity(
        id = "n1",
        appName = "Messages",
        packageName = "com.example.messages",
        sender = "A friend",
        message = "hello",
        timestamp = 1_788_720_000_000L,
        receivedAt = 1_788_720_000_000L,
    )

    @Test fun aConnectedPhoneSendsEvenIfADisconnectWasNeverCleared() = runBlocking {
        // The flag is persisted, so a phone that was disconnected once and has
        // since reconnected can carry a stale "disconnected" while a session is
        // live. Checking it before the session dropped every notification at the
        // source, with the phone showing itself connected and synced -- nothing
        // looked wrong anywhere.
        coEvery { config.get("manual_disconnect") } returns "true"
        every { client.isManuallyDisconnected() } returns true
        every { client.isConnected() } returns true
        every { client.send(any()) } returns true

        engine.send(sampleNotification)

        verify(exactly = 1) { client.send(any()) }
    }

    @Test fun aDisconnectedPhoneHoldsNotificationsInsteadOfSending() = runBlocking {
        // With no session, the disconnect still governs: the row stays pending
        // and is flushed when the user brings the phone back.
        coEvery { config.get("manual_disconnect") } returns "true"
        every { client.isManuallyDisconnected() } returns true
        every { client.isConnected() } returns false

        engine.send(sampleNotification)

        verify(exactly = 0) { client.send(any()) }
    }

    @Test fun scanningACodeConnectsWithoutAskingEvenWithTheSwitchOff() = runBlocking {
        // Scanning is the permission. Asking again straight afterwards is asking
        // the user to approve what they just did, and on a pairing with no relay
        // the switch would otherwise stop this phone dialing at all.
        coEvery { config.get(SyncEngine.AUTO_RECONNECT_KEY) } returns "false"
        coEvery { config.get("manual_disconnect") } returns "false"
        every { client.isManuallyDisconnected() } returns false
        every { client.hasPairingConsent() } returns true
        coEvery { pairings.active() } returns relayPairing
        failEveryAttempt()

        engine.connectActivePairing()

        // The local address is tried, which only happens when this phone may
        // connect without being asked...
        verify(atLeast = 1) {
            client.connect(relayPairing, any(), any(), any(), useRelay = false, requireApproval = false)
        }
        // ...and the relay attempt does not ask either.
        verify(exactly = 0) {
            client.connect(any(), any(), any(), any(), any(), requireApproval = true)
        }
    }

    @Test fun aManualDisconnectIsNotLiftedByTheReconnectionSwitch() = runBlocking {
        // Disconnect has to mean disconnect: this phone stops dialing desktops.
        // It never accepts a pending request on the user's behalf either, and it
        // never dials the saved endpoints -- the only thing it may do is wait to
        // be asked for by name, which is what awaitingRequest marks.
        coEvery { config.get(SyncEngine.AUTO_RECONNECT_KEY) } returns "true"
        coEvery { config.get("manual_disconnect") } returns "true"
        every { client.isManuallyDisconnected() } returns true
        coEvery { pairings.active() } returns relayPairing
        failEveryAttempt()

        val job = launch(start = CoroutineStart.UNDISPATCHED) { engine.maintainActivePairing() }
        try {
            delay(200)
            verify(exactly = 0) { client.acceptReconnectRequest() }
            verify(exactly = 0) {
                client.connect(any(), any(), any(), any(), any(), any(), awaitingRequest = false)
            }
        } finally {
            job.cancelAndJoin()
        }
    }

    @Test fun aDisconnectedPhoneWithTheSwitchOnReconnectsWithoutAsking() = runBlocking {
        // The switch says a PC that has connected before may reconnect without
        // asking, and it means it. Being disconnected does not turn that into a
        // prompt: this phone waits to be asked for by name, and being asked is a
        // deliberate act at the other end.
        coEvery { config.get(SyncEngine.AUTO_RECONNECT_KEY) } returns "true"
        coEvery { config.get("manual_disconnect") } returns "true"
        every { client.isManuallyDisconnected() } returns true
        coEvery { pairings.active() } returns relayPairing
        failEveryAttempt()

        val job = launch(start = CoroutineStart.UNDISPATCHED) { engine.maintainActivePairing() }
        try {
            verify(atLeast = 1) {
                client.connect(
                    relayPairing,
                    any(),
                    any(),
                    any(),
                    useRelay = true,
                    requireApproval = false,
                    awaitingRequest = true,
                )
            }
        } finally {
            job.cancelAndJoin()
        }
    }

    @Test fun aDisconnectedPhoneWithTheSwitchOffWaitsToBeAsked() = runBlocking {
        // Switched off, the same wait has to prompt. Nothing is decrypted and no
        // notification leaves this phone before the user accepts.
        coEvery { config.get(SyncEngine.AUTO_RECONNECT_KEY) } returns "false"
        coEvery { config.get("manual_disconnect") } returns "true"
        every { client.isManuallyDisconnected() } returns true
        coEvery { pairings.active() } returns relayPairing
        failEveryAttempt()

        val job = launch(start = CoroutineStart.UNDISPATCHED) { engine.maintainActivePairing() }
        try {
            verify(atLeast = 1) {
                client.connect(
                    relayPairing,
                    any(),
                    any(),
                    any(),
                    useRelay = true,
                    requireApproval = true,
                    awaitingRequest = true,
                )
            }
        } finally {
            job.cancelAndJoin()
        }
    }
}
