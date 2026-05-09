package com.focusbridge.android.pairing

import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import org.junit.Assert.assertEquals
import org.junit.Test

class PairingPayloadInputTest {
    @Test
    fun rawJsonPayloadStillWorksForManualPaste() {
        val json = """{"v":1,"mode":"local"}"""

        assertEquals(json, pairingPayloadFromInput(json))
    }

    @Test
    fun focusBridgeDeepLinkExtractsEncodedPayload() {
        val json = """{"v":1,"mode":"local"}"""
        val encoded = URLEncoder.encode(json, StandardCharsets.UTF_8.name())

        assertEquals(json, pairingPayloadFromInput("focusbridge://pair?payload=$encoded"))
    }
}
