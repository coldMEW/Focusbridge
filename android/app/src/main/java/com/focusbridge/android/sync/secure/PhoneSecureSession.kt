package com.focusbridge.android.sync.secure

/** What the transport must do with the result of feeding one relay frame in. */
sealed interface SecureStep {
    /** Send every frame, in order, as binary relay frames. */
    data class Send(val frames: List<ByteArray>) : SecureStep

    /** A complete authenticated application record. The caller must wipe it after use. */
    data class Deliver(val plaintext: ByteArray) : SecureStep

    /** An authenticated fragment of a larger record; nothing to send or deliver yet. */
    data object Continue : SecureStep
}

/**
 * Drives the phone half of `Noise_XXpsk3_25519_ChaChaPoly_SHA256` over an ordered
 * relay channel.
 *
 * The message count is fixed by the profile, so the phase is tracked here rather
 * than queried from native code: the phone writes handshake message 1, reads
 * message 2, writes message 3, then reads the desktop confirmation and writes its
 * own. Any failure closes the session permanently — the caller must drop it and
 * reconnect rather than retrying a frame, because a Noise session that has seen a
 * bad frame can no longer distinguish replay from ordinary loss.
 */
class PhoneSecureSession internal constructor(private val bridge: SecureChannelBridge) {
    private enum class Phase { NEW, AWAIT_HANDSHAKE_RESPONSE, AWAIT_DESKTOP_CONFIRMATION, READY, CLOSED }

    private var handle: Long = 0
    private var phase = Phase.NEW

    val isReady: Boolean get() = phase == Phase.READY
    val isClosed: Boolean get() = phase == Phase.CLOSED

    /**
     * Consumes and wipes [privateKey] and [enrollmentPsk]; pass dedicated copies.
     * Returns the first handshake frame, which the caller sends once the relay has
     * reported that the desktop peer is present.
     */
    fun start(
        privateKey: ByteArray,
        enrollmentPsk: ByteArray,
        pairId: ByteArray,
        desktopPublicKey: ByteArray,
    ): ByteArray {
        check(phase == Phase.NEW) { "Secure session already started" }
        return guard {
            handle = bridge.createPhone(privateKey, enrollmentPsk, pairId, desktopPublicKey)
            val frame = bridge.writeHandshake(handle)
            phase = Phase.AWAIT_HANDSHAKE_RESPONSE
            frame
        }
    }

    /** Feeds one inbound binary relay frame through the session. */
    fun receive(frame: ByteArray): SecureStep = guard {
        when (phase) {
            Phase.AWAIT_HANDSHAKE_RESPONSE -> {
                bridge.readHandshake(handle, frame)
                val final = bridge.writeHandshake(handle)
                phase = Phase.AWAIT_DESKTOP_CONFIRMATION
                SecureStep.Send(listOf(final))
            }
            Phase.AWAIT_DESKTOP_CONFIRMATION -> {
                bridge.readConfirmation(handle, frame)
                val confirmation = bridge.writeConfirmation(handle)
                // The desktop is only authenticated once its confirmation decrypts and
                // binds to this handshake; refuse to advertise readiness otherwise.
                check(bridge.isReady(handle)) { "Secure session did not reach the ready state" }
                phase = Phase.READY
                SecureStep.Send(listOf(confirmation))
            }
            Phase.READY -> {
                val plaintext = bridge.open(handle, frame)
                if (plaintext == null) SecureStep.Continue else SecureStep.Deliver(plaintext)
            }
            Phase.NEW, Phase.CLOSED -> error("Secure session is not accepting frames")
        }
    }

    /** Splits one application record into ordered relay frames. Ready state only. */
    fun seal(plaintext: ByteArray): List<ByteArray> = guard {
        check(phase == Phase.READY) { "Secure session is not ready" }
        bridge.seal(handle, plaintext).asList()
    }

    /**
     * Polls native expiry while the transport is idle, so a session that has outlived
     * its budget is torn down instead of silently accepting a later frame.
     */
    fun checkAlive(): Boolean {
        if (phase == Phase.CLOSED || handle == 0L) return false
        // isReady() runs the native liveness check first, so an expired session
        // throws here instead of accepting another frame later.
        return runCatching { guard { bridge.isReady(handle); true } }.getOrElse { false }
    }

    /** Idempotent. */
    fun close() {
        if (phase == Phase.CLOSED) return
        phase = Phase.CLOSED
        if (handle != 0L) runCatching { bridge.close(handle) }
        handle = 0
    }

    private inline fun <T> guard(block: () -> T): T =
        try {
            block()
        } catch (failure: Throwable) {
            close()
            throw failure
        }

    companion object {
        const val PAIR_ID_BYTES = 16
        const val KEY_BYTES = 32

        fun create(bridge: SecureChannelBridge = NativeSecureChannelBridge) = PhoneSecureSession(bridge)
    }
}
