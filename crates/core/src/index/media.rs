use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 媒体项信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaItem {
    /// 唯一 ID
    pub id: Uuid,

    /// 所属共享 ID
    pub share_id: Uuid,

    /// 文件名
    pub name: String,

    /// 相对路径（相对于共享根目录）
    pub relative_path: PathBuf,

    /// 完整路径
    pub full_path: PathBuf,

    /// 文件大小（字节）
    pub size: u64,

    /// 媒体类型
    pub media_type: crate::MediaType,

    /// 文件扩展名
    pub extension: String,

    /// 文件修改时间
    pub modified_at: DateTime<Utc>,

    /// 索引创建时间
    pub indexed_at: DateTime<Utc>,
}

impl MediaItem {
    pub fn new(
        share_id: Uuid,
        full_path: PathBuf,
        share_root: &PathBuf,
    ) -> Self {
        let name = full_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let relative_path = full_path
            .strip_prefix(share_root)
            .unwrap_or(&full_path)
            .to_path_buf();

        let extension = full_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let media_type = crate::MediaType::from_extension(&extension);

        // 获取文件元数据（这里简化处理）
        let size = std::fs::metadata(&full_path)
            .map(|m| m.len())
            .unwrap_or(0);

        let modified_at = std::fs::metadata(&full_path)
            .and_then(|m| m.modified())
            .map(|t| DateTime::<Utc>::from(t))
            .unwrap_or_else(|_| Utc::now());

        Self {
            id: Uuid::new_v4(),
            share_id,
            name,
            relative_path,
            full_path,
            size,
            media_type,
            extension,
            modified_at,
            indexed_at: Utc::now(),
        }
    }

    /// 获取 MIME 类型
    pub fn mime_type(&self) -> &'static str {
        self.media_type.mime_type()
    }
}

/// 媒体索引
/// 高频读取，低频写入，使用 RwLock 保护
pub struct MediaIndex {
    /// 所有媒体项，按 ID 索引
    items: std::collections::HashMap<Uuid, MediaItem>,

    /// 按 share_id 分组的索引
    by_share: std::collections::HashMap<Uuid, Vec<Uuid>>,

    /// 按路径索引（用于快速查找）
    by_path: std::collections::HashMap<PathBuf, Uuid>,
}

impl MediaIndex {
    pub fn new() -> Self {
        Self {
            items: std::collections::HashMap::new(),
            by_share: std::collections::HashMap::new(),
            by_path: std::collections::HashMap::new(),
        }
    }

    /// 从数据库加载所有媒体项
    pub async fn load_from_db(&mut self, db: &crate::db::Database) -> anyhow::Result<()> {
        let items = db.get_all_media_items().await?;

        for item in items {
            self.add_item_internal(item);
        }

        tracing::info!("Loaded {} media items from database", self.items.len());
        Ok(())
    }

    /// 添加媒体项
    pub fn add_item(&mut self, item: MediaItem) {
        self.add_item_internal(item);
    }

    fn add_item_internal(&mut self, item: MediaItem) {
        let id = item.id;
        let share_id = item.share_id;
        let path = item.full_path.clone();

        self.items.insert(id, item);

        // 更新 share 分组索引
        self.by_share
            .entry(share_id)
            .or_insert_with(Vec::new)
            .push(id);

        // 更新路径索引
        self.by_path.insert(path, id);
    }

    /// 移除媒体项
    pub fn remove_item(&mut self, id: Uuid) -> Option<MediaItem> {
        if let Some(item) = self.items.remove(&id) {
            // 从 share 分组中移除
            if let Some(share_items) = self.by_share.get_mut(&item.share_id) {
                share_items.retain(|i| *i != id);
            }

            // 从路径索引中移除
            self.by_path.remove(&item.full_path);

            Some(item)
        } else {
            None
        }
    }

    /// 根据路径移除
    pub fn remove_by_path(&mut self, path: &PathBuf) -> Option<MediaItem> {
        if let Some(id) = self.by_path.remove(path) {
            self.remove_item(id)
        } else {
            None
        }
    }

    /// 获取指定共享下的所有媒体项
    pub fn get_items_by_share(&self, share_id: Uuid) -> Vec<&MediaItem> {
        self.by_share
            .get(&share_id)
            .map(|ids| ids.iter().filter_map(|id| self.items.get(id)).collect())
            .unwrap_or_default()
    }

    /// 根据路径查找媒体项
    pub fn find_by_path(&self, path: &PathBuf) -> Option<&MediaItem> {
        self.by_path
            .get(path)
            .and_then(|id| self.items.get(id))
    }

    /// 搜索媒体项
    pub fn search(&self, query: &str) -> Vec<&MediaItem> {
        let query = query.to_lowercase();
        self.items
            .values()
            .filter(|item| item.name.to_lowercase().contains(&query))
            .collect()
    }

    /// 获取所有媒体项
    pub fn get_all_items(&self) -> Vec<&MediaItem> {
        self.items.values().collect()
    }

    /// 获取统计信息
    pub fn stats(&self) -> IndexStats {
        let total_size = self.items.values().map(|i| i.size).sum();
        let video_count = self.items.values().filter(|i| i.media_type == crate::MediaType::Video).count();
        let audio_count = self.items.values().filter(|i| i.media_type == crate::MediaType::Audio).count();
        let image_count = self.items.values().filter(|i| i.media_type == crate::MediaType::Image).count();

        IndexStats {
            total_count: self.items.len(),
            total_size,
            video_count,
            audio_count,
            image_count,
        }
    }
}

impl Default for MediaIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_count: usize,
    pub total_size: u64,
    pub video_count: usize,
    pub audio_count: usize,
    pub image_count: usize,
}