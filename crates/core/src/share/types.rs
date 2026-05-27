use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 共享文件夹配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareConfig {
    /// 共享的唯一 ID
    pub id: Uuid,

    /// 本地路径
    pub path: PathBuf,

    /// 显示名称（用户可自定义）
    pub name: String,

    /// 共享创建时间
    pub created_at: DateTime<Utc>,
}

impl ShareConfig {
    pub fn new(path: PathBuf, name: Option<String>) -> Self {
        let name = name.unwrap_or_else(|| {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unnamed Share")
                .to_string()
        });

        Self {
            id: Uuid::new_v4(),
            path,
            name,
            created_at: Utc::now(),
        }
    }
}

/// 共享文件夹运行时状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedFolder {
    pub config: ShareConfig,
    pub status: ShareStatus,
    /// 已索引的文件数量
    pub file_count: u64,
    /// 总大小（字节）
    pub total_size: u64,
}

impl SharedFolder {
    pub fn new(config: ShareConfig) -> Self {
        Self {
            config,
            status: ShareStatus::Active,
            file_count: 0,
            total_size: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShareStatus {
    Active,
    Paused,
    Error,
}