use crate::index::{MediaItem, MediaIndex};
use crate::share::SharedFolderManager;
use crate::db::Database;
use anyhow::{Context, Result};
use std::path::PathBuf;
use uuid::Uuid;
use std::collections::HashSet;

/// 文件索引扫描器
pub struct IndexScanner {
    /// 支持的媒体扩展名
    supported_extensions: HashSet<String>,
}

impl IndexScanner {
    pub fn new() -> Self {
        let extensions = [
            // Video
            "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm",
            // Audio
            "mp3", "flac", "wav", "aac", "ogg", "m4a", "wma",
            // Image
            "jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff",
        ];

        Self {
            supported_extensions: extensions
                .iter()
                .map(|s| s.to_lowercase())
                .collect(),
        }
    }

    /// 扫描单个共享文件夹
    /// 返回扫描的文件数量和总大小
    pub async fn scan_share(
        &self,
        share_id: Uuid,
        share_root: &PathBuf,
        index: &mut MediaIndex,
        db: &Database,
    ) -> Result<(u64, u64)> {
        tracing::info!("Scanning share directory: {:?}", share_root);

        // 克隆必要的数据以便在 spawn_blocking 中使用
        let share_root_clone = share_root.clone();
        let supported_extensions = self.supported_extensions.clone();

        // 使用 walkdir 进行高效目录遍历
        let items: Vec<MediaItem> = tokio::task::spawn_blocking(move || {
            use walkdir::WalkDir;
            WalkDir::new(&share_root_clone)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| {
                    e.path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| supported_extensions.contains(&ext.to_lowercase()))
                        .unwrap_or(false)
                })
                .map(|e| MediaItem::new(share_id, e.path().to_path_buf(), &share_root_clone))
                .collect()
        })
        .await
        .context("Failed to scan directory")?;

        // 事务批量插入（性能提升 10-50x）
        if !items.is_empty() {
            db.batch_insert_media_items(&items).await.context("Batch insert failed")?;
        }

        let file_count = items.len() as u64;
        let total_size = items.iter().map(|i| i.size).sum();

        // 只在内存中更新索引（锁时间极短）
        for item in &items { index.add_item(item.clone()); }

        tracing::info!(
            "Scanned {} files, total size: {} bytes",
            file_count,
            total_size
        );

        Ok((file_count, total_size))
    }

    /// 扫描单个文件（增量索引）
    pub async fn scan_single_file(
        &self,
        share_id: Uuid,
        file_path: &PathBuf,
        share_root: &PathBuf,
        index: &mut MediaIndex,
        db: &Database,
    ) -> Result<()> {
        // 检查扩展名是否支持
        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        if !extension.map(|e| self.supported_extensions.contains(&e)).unwrap_or(false) {
            return Ok(()); // 不支持的文件类型，跳过
        }

        let item = MediaItem::new(share_id, file_path.clone(), share_root);

        db.insert_media_item(&item)
            .await
            .context("Failed to insert media item")?;

        index.add_item(item);

        tracing::debug!("Indexed single file: {:?}", file_path);
        Ok(())
    }

    /// 全量扫描所有共享文件夹
    pub async fn scan_all_shares(
        &self,
        manager: &mut SharedFolderManager,
        index: &mut MediaIndex,
        db: &Database,
    ) -> Result<()> {
        // 先收集所有需要扫描的共享信息
        let shares_to_scan: Vec<(Uuid, PathBuf)> = manager
            .get_all_shares()
            .iter()
            .map(|f| (f.config.id, f.config.path.clone()))
            .collect();

        // 然后逐个扫描并更新
        for (share_id, share_root) in shares_to_scan {
            let (file_count, total_size) = self
                .scan_share(share_id, &share_root, index, db)
                .await?;

            manager.update_stats(share_id, file_count, total_size);
        }

        Ok(())
    }
}

impl Default for IndexScanner {
    fn default() -> Self {
        Self::new()
    }
}