package com.focusbridge.android.sync

import com.focusbridge.android.data.local.PairingEntity

/**
 * The line shown under the desktop status on Home.
 *
 * Its job is to make the reason for a failure obvious. A pairing saved before
 * cross-network sync existed carries no relay credentials, so off the local
 * network the phone has nothing to dial and simply retries forever. Showing the
 * saved address there is worse than useless: it looks like a network problem
 * when the real answer is one re-scan of the desktop QR.
 */
fun connectionHint(pairing: PairingEntity?, state: ConnectionState): String = when {
    pairing == null ->
        "Open desktop FocusBridge, show Pairing, then scan the QR code."
    state == ConnectionState.CONNECTED ->
        if (pairing.supportsRelay()) {
            "Connected. This desktop can also be reached from other networks."
        } else {
            pairing.endpoint
        }
    pairing.supportsRelay() ->
        "Reconnecting. This desktop can be reached from any network, so leave " +
            "FocusBridge running and it will pick up again."
    else ->
        "This pairing only works when your phone and PC are on the same Wi-Fi. " +
            "To reach this PC from mobile data, turn on cross-network sync on the " +
            "desktop and scan its new QR code."
}
