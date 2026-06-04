use std::sync::Arc;
use tokio::sync::RwLock;
use lan_media_hub_core::{FolderWatcher, IndexScanner};

/// 为单个共享文件夹启动文件监控（5 秒防抖）
pub fn spawn_watcher_for_share(
    state: Arc<RwLock<super::app_state::AppState>>,
    share_id: uuid::Uuid,
    path: std::path::PathBuf,
) {
    let debounce_dur = std::time::Duration::from_secs(5);
    tauri::async_runtime::spawn(async move {
        let mut w = match FolderWatcher::new(&path) {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!("Failed to create watcher for {}: {}", share_id, e);
                return;
            }
        };
        tracing::info!("Watching share {}: {:?}", share_id, path);
        loop {
            if w.next_event().await.is_none() { break; }
            // 防抖：清空接下来 5 秒内的事件
            let deadline = tokio::time::Instant::now() + debounce_dur;
            loop {
                match tokio::time::timeout_at(deadline, w.next_event()).await {
                    Ok(Some(_)) => continue,
                    _ => break,
                }
            }
            tracing::info!("File change in share {}, rescanning...", share_id);
            let app = state.read().await;
            if let Some(db) = &app.db {
                let mut index = app.media_index.write().await;
                let scanner = IndexScanner::new();
                if let Err(e) = scanner.scan_share(share_id, &path, &mut index, db).await {
                    tracing::warn!("Rescan of share {} failed: {}", share_id, e);
                }
            }
        }
    });
}
