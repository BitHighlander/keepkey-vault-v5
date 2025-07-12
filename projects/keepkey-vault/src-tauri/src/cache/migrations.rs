use tauri_plugin_sql::{Migration, MigrationKind};

pub fn get_cache_migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 4,
            description: "create_cache_tables",
            sql: include_str!("sql/004_cache_tables.sql"),
            kind: MigrationKind::Up,
        }
    ]
} 