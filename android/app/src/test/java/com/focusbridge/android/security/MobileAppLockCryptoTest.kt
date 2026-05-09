package com.focusbridge.android.security

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MobileAppLockCryptoTest {
    @Test
    fun verifiesOnlyMatchingSecret() {
        val salt = MobileAppLockCrypto.newSalt()
        val hash = MobileAppLockCrypto.hashSecret("1234", salt)

        assertTrue(MobileAppLockCrypto.verify("1234", salt, hash))
        assertFalse(MobileAppLockCrypto.verify("4321", salt, hash))
    }

    @Test
    fun acceptsPinOrEightCharacterPassword() {
        assertTrue(MobileAppLockCrypto.validSecret("1234"))
        assertTrue(MobileAppLockCrypto.validSecret("password"))
        assertFalse(MobileAppLockCrypto.validSecret("123"))
        assertFalse(MobileAppLockCrypto.validSecret("short"))
    }
}
