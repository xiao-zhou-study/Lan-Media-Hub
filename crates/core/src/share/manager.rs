use crate::share::{SharedFolder, ShareConfig, ShareStatus};
use crate::db::Database;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use uuid::Uuid;

/// 共享文件夹管理器
/// 负责管理所有共享文件夹的生命周期
pub struct SharedFolderManager {
    /// 当前所有共享文件夹
    shares: HashMap<Uuid, SharedFolder>,
}

impl SharedFolderManager {
    pub fn new() -> Self {
        Self {
            shares: HashMap::new(),
        }
    }

    /// 从数据库加载已有的共享配置
    pub async fn load_from_db(&mut self, db: &Database) -> Result<()> {
        let configs = db.get_all_share_configs()
            .await
            .context("Failed to load share configs from database")?;

        for config in configs {
            let mut folder = SharedFolder::new(config);
            // 从 media_index 恢复统计信息
            if let Ok((count, size)) = db.get_share_stats(folder.config.id).await {
                folder.file_count = count;
                folder.total_size = size;
            }
            self.shares.insert(folder.config.id, folder);
        }

        tracing::info!("Loaded {} shared folders from database", self.shares.len());
        Ok(())
    }

    /// 添加新的共享文件夹
    /// 返回新创建的共享 ID
    pub async fn add_share(
        &mut self,
        path: PathBuf,
        name: Option<String>,
        db: &Database,
    ) -> Result<Uuid> {
        // 校验路径
        self.validate_path(&path)?;

        // 创建配置
        let config = ShareConfig::new(path, name);
        let folder = SharedFolder::new(config.clone());

        // 保存到数据库
        db.insert_share_config(&config)
            .await
            .context("Failed to save share config to database")?;

        // 添加到内存
        let id = config.id;
        self.shares.insert(id, folder);

        tracing::info!("Added shared folder: {:?} (id: {})", config.path, id);
        Ok(id)
    }

    /// 移除共享文件夹
    pub async fn remove_share(&mut self, id: Uuid, db: &Database) -> Result<()> {
        if let Some(folder) = self.shares.remove(&id) {
            db.delete_share_config(id)
                .await
                .context("Failed to delete share config from database")?;

            // 同时删除该共享下的所有媒体索引
            db.delete_media_items_by_share(id)
                .await
                .context("Failed to delete media items for share")?;

            tracing::info!("Removed shared folder: {:?} (id: {})", folder.config.path, id);
        }
        Ok(())
    }

    /// 更新共享状态
    pub fn update_status(&mut self, id: Uuid, status: ShareStatus) {
        if let Some(folder) = self.shares.get_mut(&id) {
            folder.status = status;
        }
    }

    /// 更新统计信息
    pub fn update_stats(&mut self, id: Uuid, file_count: u64, total_size: u64) {
        if let Some(folder) = self.shares.get_mut(&id) {
            folder.file_count = file_count;
            folder.total_size = total_size;
        }
    }

    /// 获取所有共享
    pub fn get_all_shares(&self) -> Vec<&SharedFolder> {
        self.shares.values().collect()
    }

    /// 获取指定共享
    pub fn get_share(&self, id: Uuid) -> Option<&SharedFolder> {
        self.shares.get(&id)
    }

    /// 根据路径获取共享
    pub fn get_share_by_path(&self, path: &Path) -> Option<&SharedFolder> {
        self.shares.values().find(|f| f.config.path == path)
    }

    /// 获取活跃的共享数量
    pub fn active_count(&self) -> usize {
        self.shares.values()
            .filter(|f| f.status == ShareStatus::Active)
            .count()
    }

    /// 校验路径安全性
    fn validate_path(&self, path: &Path) -> Result<()> {
        // 检查路径是否存在
        if !path.exists() {
            return Err(anyhow::anyhow!("Path does not exist: {:?}", path));
        }

        // 检查是否是目录
        if !path.is_dir() {
            return Err(anyhow::anyhow!("Path is not a directory: {:?}", path));
        }

        // 清理路径，防止路径遍历
        let clean_path: PathBuf = path_clean::PathClean::clean(&path.to_path_buf());

        // 检查是否已经被共享
        if self.shares.values().any(|f| f.config.path == clean_path) {
            return Err(anyhow::anyhow!("Path is already shared: {:?}", path));
        }

        // 检查是否是子路径冲突
        for folder in self.shares.values() {
            let existing_path = &folder.config.path;
            if clean_path.starts_with(existing_path) || existing_path.starts_with(&clean_path) {
                return Err(anyhow::anyhow!(
                    "Path conflicts with existing share: {:?}",
                    existing_path
                ));
            }
        }

        Ok(())
    }

    /// 检查请求路径是否在共享范围内
    pub fn resolve_path(&self, request_path: &str) -> Option<(Uuid, PathBuf)> {
        let request_path = PathBuf::from(request_path);
        let clean_request: PathBuf = path_clean::PathClean::clean(&request_path);

        for folder in self.shares.values() {
            if folder.status != ShareStatus::Active {
                continue;
            }

            let share_path = &folder.config.path;
            if clean_request.starts_with(share_path) {
                let relative = clean_request.strip_prefix(share_path).unwrap();
                return Some((folder.config.id, relative.to_path_buf()));
            }
        }

        None
    }
}

impl Default for SharedFolderManager {
    fn default() -> Self {
        Self::new()
    }
}