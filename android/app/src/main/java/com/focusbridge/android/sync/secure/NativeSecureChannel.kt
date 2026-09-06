package com.focusbridge.android.sync.secure

/**
 * Phone-side Noise sessions. Handles are opaque IDs, never native addresses.
 * Any exception invalidates the handle: close it and reconnect, never retry a frame.
 * Transport must preserve frame order and call isReady periodically for expiry checks.
 * Library loading failure is fatal; there is no plaintext fallback.
 */
object NativeSecureChannel {
    init {
        System.loadLibrary("focusbridge_secure_channel_jni")
    }

    /** Consumes and wipes privateKey and enrollmentPsk; pass dedicated 32-byte copies.
     * Pair ID is 16 bytes and the pinned desktop public key is 32 bytes.
     * Caller must also wipe secrets in finally if native loading/invocation fails.
     */
    external fun createPhone(
        privateKey: ByteArray,
        enrollmentPsk: ByteArray,
        pairId: ByteArray,
        desktopPublicKey: ByteArray,
    ): Long

    external fun writeHandshake(handle: Long): ByteArray
    external fun readHandshake(handle: Long, frame: ByteArray)
    external fun writeConfirmation(handle: Long): ByteArray
    external fun readConfirmation(handle: Long, frame: ByteArray)
    external fun isReady(handle: Long): Boolean

    /** Nonempty plaintext, at most 1 MiB. Send every returned frame in order. */
    external fun seal(handle: Long, plaintext: ByteArray): Array<ByteArray>

    /** Null means an authenticated fragment, not yet a complete record. Wipe returned plaintext. */
    external fun open(handle: Long, frame: ByteArray): ByteArray?

    /** Idempotent, including for retired handles. */
    external fun close(handle: Long)
}
