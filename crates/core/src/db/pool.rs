use crate::db::create_schema;
use crate::share::ShareConfig;
use crate::index::MediaItem;
use crate::MediaType;
use anyhow::{Context, Result};
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::path::Path;
use uuid::Uuid;
use chrono::{DateTime, Utc};

pub struct Database { pool: SqlitePool }

impl Database {
    pub async fn new(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() { std::fs::create_dir_all(parent).context("Failed to create DB directory")?; }
        let db_url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());
        let pool = SqlitePoolOptions::new().max_connections(5).connect(&db_url).await.context("Failed to connect")?;
        create_schema(&pool).await.context("Schema creation failed")?;
        tracing::info!("Database initialized at {:?}", db_path);
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool { &self.pool }

    // ========= Share Config =========
    pub async fn insert_share_config(&self, config: &ShareConfig) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO share_configs (id, path, name, created_at, status) VALUES (?, ?, ?, ?, 'active')")
            .bind(config.id.to_string()).bind(config.path.to_string_lossy().to_string()).bind(&config.name).bind(config.created_at.to_rfc3339())
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_all_share_configs(&self) -> Result<Vec<ShareConfig>> {
        let rows: Vec<ShareConfigRow> = sqlx::query_as("SELECT id, path, name, created_at FROM share_configs WHERE status = 'active'").fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| ShareConfig {
            id: Uuid::parse_str(&r.id).unwrap(), path: Path::new(&r.path).to_path_buf(),
            name: r.name, created_at: DateTime::parse_from_rfc3339(&r.created_at).map(|d| d.with_timezone(&Utc)).unwrap(),
        }).collect())
    }

    pub async fn delete_share_config(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM share_configs WHERE id = ?").bind(id.to_string()).execute(&self.pool).await?;
        Ok(())
    }

    // ========= Media Index =========
    fn bind_media_query<'a>(q: sqlx::query::Query<'a, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'a>>, item: &'a MediaItem) -> sqlx::query::Query<'a, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'a>> {
        q.bind(item.id.to_string()).bind(item.share_id.to_string()).bind(&item.name)
         .bind(item.relative_path.to_string_lossy().to_string()).bind(item.full_path.to_string_lossy().to_string())
         .bind(item.size as i64)
         .bind(match item.media_type { MediaType::Video=>"video", MediaType::Audio=>"audio", MediaType::Image=>"image", MediaType::Other=>"other" })
         .bind(&item.extension).bind(item.modified_at.to_rfc3339()).bind(item.indexed_at.to_rfc3339())
    }

    pub async fn insert_media_item(&self, item: &MediaItem) -> Result<()> {
        let q = sqlx::query("INSERT INTO media_index (id,share_id,name,relative_path,full_path,size,media_type,extension,modified_at,indexed_at) VALUES (?,?,?,?,?,?,?,?,?,?)");
        Self::bind_media_query(q, item).execute(&self.pool).await?;
        Ok(())
    }

    /// 事务批量插入（性能提升 10-50x，减少锁持有时间）
    pub async fn batch_insert_media_items(&self, items: &[MediaItem]) -> Result<()> {
        if items.is_empty() { return Ok(()); }
        let mut tx = self.pool.begin().await?;
        for item in items {
            let q = sqlx::query("INSERT INTO media_index (id,share_id,name,relative_path,full_path,size,media_type,extension,modified_at,indexed_at) VALUES (?,?,?,?,?,?,?,?,?,?)");
            Self::bind_media_query(q, item).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        tracing::debug!("Batch inserted {} media items", items.len());
        Ok(())
    }

    pub async fn get_all_media_items(&self) -> Result<Vec<MediaItem>> {
        let rows: Vec<MediaItemRow> = sqlx::query_as("SELECT id,share_id,name,relative_path,full_path,size,media_type,extension,modified_at,indexed_at FROM media_index").fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(row_to_item).collect())
    }

    pub async fn get_share_stats(&self, share_id: Uuid) -> Result<(u64, u64)> {
        let row: (i64, i64) = sqlx::query_as("SELECT COUNT(*), COALESCE(SUM(size),0) FROM media_index WHERE share_id = ?").bind(share_id.to_string()).fetch_one(&self.pool).await?;
        Ok((row.0 as u64, row.1 as u64))
    }

    pub async fn get_media_items_by_share(&self, share_id: Uuid) -> Result<Vec<MediaItem>> {
        let rows: Vec<MediaItemRow> = sqlx::query_as("SELECT id,share_id,name,relative_path,full_path,size,media_type,extension,modified_at,indexed_at FROM media_index WHERE share_id = ?").bind(share_id.to_string()).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(row_to_item).collect())
    }

    pub async fn delete_media_items_by_share(&self, share_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM media_index WHERE share_id = ?").bind(share_id.to_string()).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn delete_media_item(&self, full_path: &Path) -> Result<()> {
        sqlx::query("DELETE FROM media_index WHERE full_path = ?").bind(full_path.to_string_lossy().to_string()).execute(&self.pool).await?;
        Ok(())
    }

    // ========= Key-Value Store =========
    pub async fn get_kv(&self, key: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as("SELECT value FROM kv_store WHERE key = ?")
            .bind(key).fetch_optional(&self.pool).await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn set_kv(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO kv_store (key, value) VALUES (?, ?)")
            .bind(key).bind(value).execute(&self.pool).await?;
        Ok(())
    }
}

fn row_to_item(r: MediaItemRow) -> MediaItem {
    MediaItem {
        id: Uuid::parse_str(&r.id).unwrap(), share_id: Uuid::parse_str(&r.share_id).unwrap(),
        name: r.name, relative_path: Path::new(&r.relative_path).to_path_buf(), full_path: Path::new(&r.full_path).to_path_buf(),
        size: r.size as u64,
        media_type: match r.media_type.as_str() { "video"=>MediaType::Video, "audio"=>MediaType::Audio, "image"=>MediaType::Image, _=>MediaType::Other },
        extension: r.extension,
        modified_at: DateTime::parse_from_rfc3339(&r.modified_at).map(|d| d.with_timezone(&Utc)).unwrap(),
        indexed_at: DateTime::parse_from_rfc3339(&r.indexed_at).map(|d| d.with_timezone(&Utc)).unwrap(),
    }
}

#[derive(sqlx::FromRow)] struct ShareConfigRow { id: String, path: String, name: String, created_at: String }
#[derive(sqlx::FromRow)] struct MediaItemRow { id: String, share_id: String, name: String, relative_path: String, full_path: String, size: i64, media_type: String, extension: String, modified_at: String, indexed_at: String }