CREATE TABLE IF NOT EXISTS notifications (
    id TEXT PRIMARY KEY,
    app_name TEXT NOT NULL,
    package_name TEXT NOT NULL,
    sender TEXT NOT NULL,
    message TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    received_at INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'NEW',
    priority INTEGER NOT NULL DEFAULT 30,
    content_hidden INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_notifications_status ON notifications(status);
CREATE INDEX IF NOT EXISTS idx_notifications_timestamp ON notifications(timestamp);
CREATE INDEX IF NOT EXISTS idx_notifications_package ON notifications(package_name);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS paired_devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_name TEXT NOT NULL,
    device_id TEXT NOT NULL UNIQUE,
    pairing_key TEXT NOT NULL,
    mode TEXT NOT NULL DEFAULT 'LOCAL',
    endpoint TEXT,
    cert_fingerprint TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    last_connected_at INTEGER
);

CREATE TABLE IF NOT EXISTS app_rules (
    package_name TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT 'other',
    notifications_seen INTEGER NOT NULL DEFAULT 0,
    last_seen_at INTEGER NOT NULL DEFAULT 0,
    muted INTEGER NOT NULL DEFAULT 0,
    priority INTEGER NOT NULL DEFAULT 0,
    study_safe INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_app_rules_category ON app_rules(category);
CREATE INDEX IF NOT EXISTS idx_app_rules_muted ON app_rules(muted);
