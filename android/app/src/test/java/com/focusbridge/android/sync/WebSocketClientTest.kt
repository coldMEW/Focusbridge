package com.focusbridge.android.sync

import android.content.Context
import com.focusbridge.android.data.local.PairingEntity
import com.focusbridge.android.data.local.AppRuleEntity
import com.focusbridge.android.data.repository.AppRuleRepository
import com.focusbridge.android.data.repository.ConfigRepository
import io.mockk.*
import kotlinx.coroutines.CompletableDeferred
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import okhttp3.*
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test

class WebSocketClientTest {
    private val transport = mockk<OkHttpClient>()
    private val inventory = mockk<AppInventoryProvider>()
    private val config = mockk<ConfigRepository>(relaxed = true)
    private val appRules = mockk<AppRuleRepository>(relaxed = true)
    private val sockets = mutableListOf<WebSocket>()
    private val listeners = mutableListOf<WebSocketListener>()
    private lateinit var client: WebSocketClient
    private val pairing = PairingEntity("desktop", "wss://localhost:1234", pairingKey = "a".repeat(64), certFingerprint = "b".repeat(64))

    @Before fun setup() {
        mockkStatic("com.focusbridge.android.sync.PinnedTlsKt")
        every { transport.withPinnedCertificate(any()) } returns transport
        every { transport.newWebSocket(any(), any()) } answers {
            listeners += secondArg<WebSocketListener>()
            mockk<WebSocket>(relaxed = true).also {
                every { it.send(any<String>()) } returns true
                sockets += it
            }
        }
        every { inventory.launchableApps() } returns emptyList()
        client = WebSocketClient(mockk<Context>(relaxed = true), transport, inventory, appRules, config, mockk(relaxed = true), mockk(relaxed = true))
    }

    @After fun cleanup() { client.disconnect(); unmockkStatic("com.focusbridge.android.sync.PinnedTlsKt") }
    private fun connect() = client.connect(pairing, deviceName = "Phone")
    private fun auth(index: Int) = listeners[index].onMessage(sockets[index], "{\"type\":\"AUTH_OK\",\"payload\":{}}")

    @Test fun manualDisconnectBlocksConnectBeforePreferenceWriteCompletes() {
        val blocked = CompletableDeferred<Unit>()
        coEvery { config.set("manual_disconnect", "true") } coAnswers { blocked.await() }
        connect()
        client.manualDisconnect()
        connect()
        assertEquals(1, sockets.size)
        assertEquals(ConnectionState.DISCONNECTED, client.state.value)
        blocked.complete(Unit)
    }

    @Test fun lateAuthenticationAfterFailureCannotReviveSocket() {
        connect()
        listeners[0].onFailure(sockets[0], IllegalStateException("closed"), null)
        auth(0)
        assertFalse(client.isConnected())
        verify(exactly = 0) { inventory.launchableApps() }
    }

    @Test fun inventoryFailureDoesNotEscapeAuthenticationOrSendEmptyReplacement() {
        every { inventory.launchableApps() } throws IllegalStateException("package query failed")
        connect()
        auth(0)
        assertTrue(client.isConnected())
        verify(timeout = 2000) { inventory.launchableApps() }
        verify(exactly = 0) { sockets[0].send(any<String>()) }
    }

    @Test fun everyAuthenticationCollectsFreshInventory() {
        connect()
        auth(0)
        verify(timeout = 2000, exactly = 1) { inventory.launchableApps() }
        connect()
        auth(1)
        verify(timeout = 2000, exactly = 2) { inventory.launchableApps() }
    }

    @Test fun remoteClosingInvalidatesSessionBeforeClosedCallback() {
        connect()
        listeners[0].onClosing(sockets[0], 1000, "bye")
        auth(0)
        assertFalse(client.isConnected())
    }

    @Test fun oldCallbacksCannotDisconnectOrUnpairReplacement() {
        connect()
        connect()
        auth(1)
        listeners[0].onFailure(sockets[0], IllegalStateException("old"), null)
        listeners[0].onMessage(sockets[0], "{\"type\":\"UNPAIR\",\"payload\":{}}")
        assertTrue(client.isConnected())
        coVerify(exactly = 0) { config.set("manual_disconnect", "true") }
    }

    @Test fun slowInventoryCannotPublishAfterDisconnect() {
        val started = CountDownLatch(1)
        val release = CountDownLatch(1)
        val finished = CountDownLatch(1)
        every { inventory.launchableApps() } answers {
            started.countDown()
            check(release.await(2, TimeUnit.SECONDS))
            finished.countDown()
            emptyList()
        }
        connect()
        auth(0)
        try {
            assertTrue(started.await(2, TimeUnit.SECONDS))
            client.disconnect()
        } finally {
            release.countDown()
        }
        assertTrue(finished.await(2, TimeUnit.SECONDS))
        verify(exactly = 0) { sockets[0].send(any<String>()) }
    }

    @Test fun reconnectPreferenceWritesFollowIntentOrder() {
        val blocked = CompletableDeferred<Unit>()
        val writes = java.util.Collections.synchronizedList(mutableListOf<String>())
        coEvery { config.set("manual_disconnect", any()) } coAnswers {
            val value = secondArg<String>()
            if (value == "true") blocked.await()
            writes += value
        }
        client.manualDisconnect()
        client.acceptReconnectRequest()
        blocked.complete(Unit)
        coVerify(timeout = 2000) { config.set("manual_disconnect", "false") }
        // Acquiring the client lock also verifies that reconnect intent is immediately usable.
        connect()
        assertEquals(1, sockets.size)
        assertEquals(listOf("true", "false"), writes.toList())
    }

    @Test fun laterRulesSnapshotWinsWhenInitialDatabaseWriteIsDelayed() {
        val firstStarted = CountDownLatch(1)
        val secondStarted = CountDownLatch(1)
        val completed = CountDownLatch(2)
        val releaseFirst = CompletableDeferred<Unit>()
        val writes = java.util.concurrent.CopyOnWriteArrayList<String>()
        coEvery { appRules.replaceFromDesktop(any()) } coAnswers {
            val muted = firstArg<List<AppRuleEntity>>().single().muted
            if (!muted) {
                firstStarted.countDown()
                releaseFirst.await()
            } else {
                secondStarted.countDown()
            }
            writes += "rules:$muted"
        }
        coEvery { config.set("favorite_contacts", any()) } coAnswers {
            writes += "contacts:${secondArg<String>()}"
            completed.countDown()
        }
        connect()
        try {
            rules(false, "initial")
            assertTrue(firstStarted.await(2, TimeUnit.SECONDS))
            rules(true, "snapshot")
            // Give an incorrectly concurrent second update a chance to overtake the first.
            secondStarted.await(300, TimeUnit.MILLISECONDS)
        } finally {
            releaseFirst.complete(Unit)
        }
        assertTrue(completed.await(2, TimeUnit.SECONDS))
        assertEquals(listOf("rules:false", "contacts:initial", "rules:true", "contacts:snapshot"), writes.toList())
    }

    private fun rules(muted: Boolean, contact: String) = listeners.last().onMessage(
        sockets.last(),
        """{"type":"RULES_UPDATE","payload":{"appRules":[{"packageName":"com.example","muted":$muted}],"favoriteContacts":["$contact"]}}""",
    )

    @Test fun disconnectCancelsQueuedRulesAndAllowsReplacementSessionUpdate() {
        val firstStarted = CountDownLatch(1)
        val firstCancelled = CountDownLatch(1)
        val neverReleased = CompletableDeferred<Unit>()
        val completed = CountDownLatch(1)
        val writes = java.util.concurrent.CopyOnWriteArrayList<String>()
        coEvery { appRules.replaceFromDesktop(any()) } coAnswers {
            if (!firstArg<List<AppRuleEntity>>().single().muted) {
                firstStarted.countDown()
                try {
                    neverReleased.await()
                } finally {
                    firstCancelled.countDown()
                }
            }
        }
        coEvery { config.set("favorite_contacts", any()) } coAnswers {
            writes += secondArg<String>()
            completed.countDown()
        }
        connect()
        rules(false, "initial")
        assertTrue(firstStarted.await(2, TimeUnit.SECONDS))
        rules(true, "cancelled snapshot")
        client.disconnect()
        assertTrue(firstCancelled.await(2, TimeUnit.SECONDS))
        connect()
        rules(true, "replacement")
        assertTrue(completed.await(2, TimeUnit.SECONDS))
        assertEquals(listOf("replacement"), writes.toList())
        coVerify(exactly = 2) { appRules.replaceFromDesktop(any()) }
    }
}
