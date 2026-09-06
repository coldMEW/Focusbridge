package com.focusbridge.android.storage

import android.content.Context
import com.focusbridge.android.security.DatabaseKeyStore
import java.util.concurrent.Callable
import java.util.concurrent.Executors
import java.util.concurrent.Future

/**
 * Runs the encrypted-storage preparation once, off the main thread.
 *
 * Preparation opens files, may load SQLCipher and, on the first launch after an
 * upgrade, exports the whole database. That is far too much work for the main
 * thread, but the database provider can be first injected from anywhere. So the
 * work is started as the process comes up and whoever needs the database waits
 * for the same result rather than starting a second migration.
 *
 * The passphrase is deliberately not exposed: callers get it exactly once,
 * through [take], because the Room factory that receives it owns wiping it.
 */
object DatabasePreparation {
    private val lock = Any()
    private var pending: Future<ByteArray>? = null
    private var taken = false

    /** Starts preparation in the background. Safe to call more than once. */
    fun start(context: Context, name: String) {
        val application = context.applicationContext
        synchronized(lock) {
            if (pending != null) return
            val executor = Executors.newSingleThreadExecutor { runnable ->
                Thread(runnable, "focusbridge-db-init").apply { isDaemon = true }
            }
            pending = executor.submit(
                Callable {
                    EncryptedDatabaseMigrator(application.getDatabasePath(name))
                        .prepare(DatabaseKeyStore(application))
                },
            )
            executor.shutdown()
        }
    }

    /**
     * Waits for preparation and returns the passphrase. The caller takes
     * ownership and must wipe it. Any failure propagates: there is no plaintext
     * fallback, because falling back would silently un-encrypt the user's data.
     */
    fun take(context: Context, name: String): ByteArray {
        start(context, name)
        val future = synchronized(lock) {
            check(!taken) { "The database passphrase has already been taken" }
            taken = true
            requireNotNull(pending)
        }
        return try {
            future.get()
        } catch (failure: java.util.concurrent.ExecutionException) {
            // Surface the real cause; the wrapper carries no useful information.
            throw failure.cause ?: failure
        }
    }

    /** Test seam: forget any prepared state so a fixture can prepare again. */
    internal fun resetForTest() {
        synchronized(lock) {
            pending = null
            taken = false
        }
    }
}
