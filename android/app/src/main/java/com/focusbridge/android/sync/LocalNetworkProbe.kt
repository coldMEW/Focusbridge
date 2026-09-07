package com.focusbridge.android.sync

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Whether this phone is on a network where the desktop's local address could
 * possibly answer.
 *
 * The saved LAN addresses are private ones -- 192.168.x, 10.x and the like. On
 * mobile data none of them is reachable, but the phone tried each in turn and
 * waited four seconds for every one before falling through to the relay. With
 * two or three addresses saved that is eight to twelve seconds of certain
 * failure on every single connection, which is most of the delay a user sees
 * after scanning a code away from home.
 */
interface LocalNetworkProbe {
    fun hasLocalNetwork(): Boolean
}

@Singleton
class AndroidLocalNetworkProbe @Inject constructor(
    @ApplicationContext private val context: Context,
) : LocalNetworkProbe {
    override fun hasLocalNetwork(): Boolean {
        val manager = context.getSystemService(ConnectivityManager::class.java) ?: return true
        val network = manager.activeNetwork ?: return false
        val capabilities = manager.getNetworkCapabilities(network) ?: return true
        // Wi-Fi, Ethernet and a phone acting as a hotspot can all reach a desktop
        // on the same network. Anything else cannot, and should not be waited on.
        return capabilities.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) ||
            capabilities.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET)
    }
}

@Module
@InstallIn(SingletonComponent::class)
abstract class LocalNetworkProbeModule {
    @Binds
    abstract fun bind(probe: AndroidLocalNetworkProbe): LocalNetworkProbe
}
