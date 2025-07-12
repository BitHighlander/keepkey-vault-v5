pub struct DeviceInfo {
    pub id: i64,
    pub device_id: String,
    pub label: Option<String>,
    pub features_json: String,
    pub last_seen: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XpubInfo {
    pub id: i64,
    pub device_id: String,
    pub path: String,
    pub label: String,
    pub caip: String,
    pub pubkey: String,
    pub created_at: i64,
}

pub struct Database;

impl Database {
    pub fn get_migrations() -> Vec<Migration> {
        vec![
            Migration {
                version: 1,
                description: "create_devices_table",
                sql: "CREATE TABLE IF NOT EXISTS devices (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    device_id TEXT NOT NULL UNIQUE,
                    label TEXT,
                    features_json TEXT NOT NULL,
                    last_seen INTEGER NOT NULL,
                    created_at INTEGER NOT NULL
                );",
                kind: MigrationKind::Up,
            },
            Migration {
                version: 2,
                description: "create_xpubs_table", 
                sql: "CREATE TABLE IF NOT EXISTS xpubs (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    device_id TEXT NOT NULL,
                    path TEXT NOT NULL,
                    label TEXT NOT NULL,
                    caip TEXT NOT NULL,
                    pubkey TEXT NOT NULL,
                    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                    UNIQUE(device_id, path, caip),
                    FOREIGN KEY (device_id) REFERENCES devices(device_id) ON DELETE CASCADE
                );",
                kind: MigrationKind::Up,
            },
            Migration {
                version: 3,
                description: "create_indexes",
                sql: "CREATE INDEX IF NOT EXISTS idx_devices_device_id ON devices(device_id);
                       CREATE INDEX IF NOT EXISTS idx_xpubs_device_id ON xpubs(device_id);
                       CREATE INDEX IF NOT EXISTS idx_xpubs_lookup ON xpubs(device_id, path, caip);",
                kind: MigrationKind::Up,
            },
        ]
    }
} 