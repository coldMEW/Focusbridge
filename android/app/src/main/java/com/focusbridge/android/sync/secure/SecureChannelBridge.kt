package com.focusbridge.android.sync.secure

/**
 * The nine native operations, behind an interface so transport logic is unit-testable
 * on the JVM where the .so cannot be loaded. The only production implementation is
 * [NativeSecureChannelBridge]; a fake must never be reachable from application code.
 */
interface SecureChannelBridge {
    fun createPhone(
        privateKey: ByteArray,
        enrollmentPsk: ByteArray,
        pairId: ByteArray,
        desktopPublicKey: ByteArray,
    ): Long

    fun writeHandshake(handle: Long): ByteArray
    fun readHandshake(handle: Long, frame: ByteArray)
    fun writeConfirmation(handle: Long): ByteArray
    fun readConfirmation(handle: Long, frame: ByteArray)
    fun isReady(handle: Long): Boolean
    fun seal(handle: Long, plaintext: ByteArray): Array<ByteArray>
    fun open(handle: Long, frame: ByteArray): ByteArray?
    fun close(handle: Long)
}

/**
 * Loads the shared Rust engine. Library loading failure is fatal for the relay
 * transport: there is no plaintext fallback and no software reimplementation.
 */
object NativeSecureChannelBridge : SecureChannelBridge {
    override fun createPhone(
        privateKey: ByteArray,
        enrollmentPsk: ByteArray,
        pairId: ByteArray,
        desktopPublicKey: ByteArray,
    ): Long = NativeSecureChannel.createPhone(privateKey, enrollmentPsk, pairId, desktopPublicKey)

    override fun writeHandshake(handle: Long): ByteArray = NativeSecureChannel.writeHandshake(handle)
    override fun readHandshake(handle: Long, frame: ByteArray) = NativeSecureChannel.readHandshake(handle, frame)
    override fun writeConfirmation(handle: Long): ByteArray = NativeSecureChannel.writeConfirmation(handle)
    override fun readConfirmation(handle: Long, frame: ByteArray) = NativeSecureChannel.readConfirmation(handle, frame)
    override fun isReady(handle: Long): Boolean = NativeSecureChannel.isReady(handle)
    override fun seal(handle: Long, plaintext: ByteArray): Array<ByteArray> = NativeSecureChannel.seal(handle, plaintext)
    override fun open(handle: Long, frame: ByteArray): ByteArray? = NativeSecureChannel.open(handle, frame)
    override fun close(handle: Long) = NativeSecureChannel.close(handle)
}
