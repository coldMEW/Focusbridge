package com.focusbridge.android.security

import java.io.File
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

// JVM guardrails for security boundaries in Compose; device lifecycle testing is still required.
class MobileAppLockWiringTest {
    private val source = File("src/main/java/com/focusbridge/android/MainActivity.kt").readText()

    private fun field(value: String): String = requireNotNull(
        Regex("OutlinedTextField\\(\\s*value = ${Regex.escape(value)},[\\s\\S]*?\\n\\s*\\)").find(source),
    ) { "Missing field: $value" }.value

    @Test fun allLocalSecretsAreMaskedWithPasswordKeyboard() {
        listOf("secret", "answer", "newSecret", "lockSecret", "lockAnswer").forEach { value ->
            val field = field(value)
            assertTrue("$value must be masked", field.contains("visualTransformation = PasswordVisualTransformation()"))
            assertTrue("$value must use password input", field.contains("keyboardType = KeyboardType.Password"))
            assertTrue("$value must disable autocorrect", field.contains("autoCorrect = false"))
        }
    }

    @Test fun securityQuestionRemainsVisible() {
        assertTrue(source.contains("Text(recoveryQuestion ?:"))
        assertTrue(source.contains("Text(\"Security question\""))
        assertFalse(field("customLockQuestion").contains("PasswordVisualTransformation"))
    }

    @Test fun loadingAndMissingHashCannotBypassGate() {
        assertTrue(source.contains("if (lockConfig == null)"))
        assertFalse(source.contains("appLockEnabled && !appUnlocked && !appLockHash.isNullOrBlank()"))
    }

    @Test fun backgroundingRevokesUnlock() {
        assertTrue(source.contains("Lifecycle.Event.ON_PAUSE"))
        assertTrue(source.contains("lockSession.relock()"))
    }

    @Test fun pairingIsNotConsumedBeforeAuthentication() {
        assertFalse(source.contains("consumePairingIntent(intent)"))
        // The app-lock gate returns before the pairing handler is reached, so a
        // locked phone cannot be re-paired by an incoming link.
        assertTrue(
            source.indexOf("if (appLockEnabled && !appUnlocked)") <
                source.indexOf("val pairingRequest = pendingPairing"),
        )
    }

    @Test fun pairingLinksRequireExplicitConfirmation() {
        // Any installed app or web page can send focusbridge://pair, and pairing
        // grants a PC access to this phone's notifications. The payload must only
        // be consumed from a confirmation button, never from a launched effect.
        val consume = source.indexOf("pairingManager.consume(pairingRequest)")
        assertTrue(consume > 0)
        val confirm = source.lastIndexOf("onClick = {", consume)
        assertTrue(confirm in 1 until consume)
        assertFalse(source.contains("LaunchedEffect(pendingPairing"))
        // The user is shown what they are connecting to before they can accept.
        assertTrue(source.contains("preview.shortFingerprint()"))
        assertTrue(source.contains("preview.crossNetwork"))
    }

    @Test fun reconnectPromptIsBehindGate() {
        assertTrue(source.contains("if (reconnectRequest != null && (!appLockEnabled || appUnlocked))"))
    }

    @Test fun bothCredentialsUseSharedLimiter() {
        assertTrue(source.contains("lockAttempts.verify(secret"))
        assertTrue(source.contains("lockAttempts.verify(answer.lowercase()"))
    }
}
