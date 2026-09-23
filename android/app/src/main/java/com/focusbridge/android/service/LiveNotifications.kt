package com.focusbridge.android.service

import java.util.concurrent.ConcurrentHashMap

/**
 * What each notification now in the shade last said.
 *
 * Apps re-post a notification to update it far more often than they post a new
 * one: Spotify and Pocket Casts on every play, pause, seek and progress tick,
 * WhatsApp whenever a chat changes. Each re-post arrived here as if it were new,
 * and the desktop inbox filled with the same message over and over. A re-post
 * under the same key that says exactly what it said before is an update, not a
 * message. Once the notification is dismissed the key is forgotten, so the same
 * words posted again later are new again -- the same rule the shade follows.
 *
 * Kept in memory only. After a restart the shade is re-read when the listener
 * connects, so what is already showing is not captured a second time.
 */
internal class LiveNotifications {
    private val shown = ConcurrentHashMap<String, String>()

    /**
     * Records what [key] now says, and answers whether that is exactly what it
     * said already -- an update with nothing new in it.
     */
    fun isRepeat(key: String, signature: String): Boolean = shown.put(key, signature) == signature

    /** Takes note of a notification already on screen without treating it as new. */
    fun remember(key: String, signature: String) {
        shown[key] = signature
    }

    fun forget(key: String) {
        shown.remove(key)
    }
}
