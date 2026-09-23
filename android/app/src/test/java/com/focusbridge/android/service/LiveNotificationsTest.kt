package com.focusbridge.android.service

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LiveNotificationsTest {
    @Test
    fun anUpdateThatSaysNothingNewIsARepeat() {
        // Spotify re-posting the same track as it plays.
        val live = LiveNotifications()
        assertFalse(live.isRepeat("spotify:1", "song-a"))
        assertTrue(live.isRepeat("spotify:1", "song-a"))
        assertTrue(live.isRepeat("spotify:1", "song-a"))
    }

    @Test
    fun newContentUnderTheSameKeyIsNew() {
        // The next track, or a new WhatsApp message in the same chat.
        val live = LiveNotifications()
        assertFalse(live.isRepeat("spotify:1", "song-a"))
        assertFalse(live.isRepeat("spotify:1", "song-b"))
        assertTrue(live.isRepeat("spotify:1", "song-b"))
    }

    @Test
    fun theSameWordsAfterADismissalAreNew() {
        val live = LiveNotifications()
        assertFalse(live.isRepeat("app:7", "Your order is here"))
        live.forget("app:7")
        assertFalse(live.isRepeat("app:7", "Your order is here"))
    }

    @Test
    fun differentKeysDoNotSuppressEachOther() {
        val live = LiveNotifications()
        assertFalse(live.isRepeat("chat:asha", "hi"))
        assertFalse(live.isRepeat("chat:ravi", "hi"))
    }

    @Test
    fun whatWasAlreadyShowingAtStartIsNotNew() {
        // After a restart the shade is re-read, so a notification still showing
        // is not captured again on its next unchanged update.
        val live = LiveNotifications()
        live.remember("podcast:1", "episode-3")
        assertTrue(live.isRepeat("podcast:1", "episode-3"))
    }
}
