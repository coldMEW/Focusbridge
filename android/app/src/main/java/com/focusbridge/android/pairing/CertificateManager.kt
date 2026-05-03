package com.focusbridge.android.pairing

import javax.inject.Inject

class CertificateManager @Inject constructor() {
    fun isExpectedFingerprint(actualSha256: String, expectedSha256: String): Boolean =
        actualSha256.equals(expectedSha256, ignoreCase = true)
}
