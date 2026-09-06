package com.focusbridge.android.security

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MobileLockSessionTest {
    @Test fun startsLockedAndRelocksAfterUnlock() {
        val session = MobileLockSession()
        assertFalse(session.unlocked)
        session.unlock(session.generation)
        assertTrue(session.unlocked)
        session.relock()
        assertFalse(session.unlocked)
    }

    @Test fun inFlightAuthenticationCannotUndoRelock() {
        val session = MobileLockSession()
        val generation = session.generation
        session.relock()
        session.unlock(generation)
        assertFalse(session.unlocked)
        session.unlock(session.generation)
        assertTrue(session.unlocked)
    }
}
