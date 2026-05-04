package com.focusbridge.android.sync

import java.security.MessageDigest
import java.security.cert.X509Certificate
import javax.net.ssl.SSLContext
import javax.net.ssl.TrustManager
import javax.net.ssl.X509TrustManager
import okhttp3.OkHttpClient

fun OkHttpClient.withPinnedCertificate(expectedSha256Hex: String): OkHttpClient {
    val trustManager = PinningTrustManager(expectedSha256Hex)
    val sslContext = SSLContext.getInstance("TLS")
    sslContext.init(null, arrayOf<TrustManager>(trustManager), null)
    return newBuilder()
        .sslSocketFactory(sslContext.socketFactory, trustManager)
        .hostnameVerifier { _, session ->
            val cert = session.peerCertificates.firstOrNull() as? X509Certificate ?: return@hostnameVerifier false
            trustManager.matches(cert)
        }
        .build()
}

class PinningTrustManager(
    private val expectedSha256Hex: String,
) : X509TrustManager {
    override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?) = Unit

    override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?) {
        val cert = chain?.firstOrNull() ?: throw java.security.cert.CertificateException("missing server cert")
        if (!matches(cert)) {
            throw java.security.cert.CertificateException("FocusBridge certificate pin mismatch")
        }
    }

    override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()

    fun matches(cert: X509Certificate): Boolean {
        val digest = MessageDigest.getInstance("SHA-256").digest(cert.encoded)
        return digest.joinToString("") { "%02x".format(it) }.equals(expectedSha256Hex, ignoreCase = true)
    }
}
