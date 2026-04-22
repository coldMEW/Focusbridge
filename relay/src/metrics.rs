use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct Metrics {
    pub connections_total: AtomicU64,
    pub messages_routed_total: AtomicU64,
    pub messages_queued_total: AtomicU64,
    pub messages_dropped_total: AtomicU64,
    pub auth_failures_total: AtomicU64,
}

impl Metrics {
    pub fn render(&self) -> String {
        let lines = [
            format!(
                "# HELP focusbridge_connections_total Total websocket connections accepted\n# TYPE focusbridge_connections_total counter\nfocusbridge_connections_total {}",
                self.connections_total.load(Ordering::Relaxed)
            ),
            format!(
                "# HELP focusbridge_messages_routed_total Messages delivered live\n# TYPE focusbridge_messages_routed_total counter\nfocusbridge_messages_routed_total {}",
                self.messages_routed_total.load(Ordering::Relaxed)
            ),
            format!(
                "# HELP focusbridge_messages_queued_total Messages queued pending peer\n# TYPE focusbridge_messages_queued_total counter\nfocusbridge_messages_queued_total {}",
                self.messages_queued_total.load(Ordering::Relaxed)
            ),
            format!(
                "# HELP focusbridge_messages_dropped_total Messages dropped\n# TYPE focusbridge_messages_dropped_total counter\nfocusbridge_messages_dropped_total {}",
                self.messages_dropped_total.load(Ordering::Relaxed)
            ),
            format!(
                "# HELP focusbridge_auth_failures_total Authentication failures\n# TYPE focusbridge_auth_failures_total counter\nfocusbridge_auth_failures_total {}",
                self.auth_failures_total.load(Ordering::Relaxed)
            ),
        ];
        lines.join("\n") + "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_contains_all_counters() {
        let m = Metrics::default();
        m.connections_total.fetch_add(2, Ordering::Relaxed);
        let out = m.render();
        assert!(out.contains("focusbridge_connections_total 2"));
        assert!(out.contains("focusbridge_messages_routed_total 0"));
    }
}
