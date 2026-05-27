use crate::db::DatabaseError;

/// 分条创建数据库 schema（sqlx 不支持多语句执行）
pub async fn create_schema(pool: &sqlx::SqlitePool) -> Result<(), DatabaseError> {
    let statements = [
        "CREATE TABLE IF NOT EXISTS share_configs (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active'
        )",
        "CREATE TABLE IF NOT EXISTS media_index (
            id TEXT PRIMARY KEY,
            share_id TEXT NOT NULL,
            name TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            full_path TEXT NOT NULL UNIQUE,
            size INTEGER NOT NULL,
            media_type TEXT NOT NULL,
            extension TEXT NOT NULL,
            modified_at TEXT NOT NULL,
            indexed_at TEXT NOT NULL,
            FOREIGN KEY (share_id) REFERENCES share_configs(id) ON DELETE CASCADE
        )",
        "CREATE INDEX IF NOT EXISTS idx_media_share_id ON media_index(share_id)",
        "CREATE INDEX IF NOT EXISTS idx_media_name ON media_index(name)",
        "CREATE INDEX IF NOT EXISTS idx_media_full_path ON media_index(full_path)",
        // Key-value store for settings (password, JWT secret, etc.)
        "CREATE TABLE IF NOT EXISTS kv_store (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
    ];

    for (i, stmt) in statements.iter().enumerate() {
        sqlx::query(stmt).execute(pool).await.map_err(|e| {
            tracing::error!("Schema statement {} failed: {}", i, e);
            DatabaseError::MigrationError(format!("stmt {}: {}", i, e))
        })?;
    }

    tracing::info!("Database schema created successfully");
    Ok(())
}