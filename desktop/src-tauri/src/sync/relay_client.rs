// Cloud-mode relay client stub. Connects to the relay over WSS using
// `tokio-tungstenite` with native roots, authenticates with the pairing key,
// and forwards envelopes into the same handler used by the local WS server.
//
// Runtime wiring deferred to Phase 5 (cloud relay deploy).

#[derive(Debug, Clone)]
pub struct RelayConfig {
    pub url: String,
    pub pairing_key: String,
    pub device_pair_id: String,
}
