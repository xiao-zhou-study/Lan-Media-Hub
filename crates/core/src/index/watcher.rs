use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher, Config};
use tokio::sync::mpsc;
use std::path::PathBuf;
use anyhow::Result;

/// 文件变化事件
#[derive(Debug, Clone)]
pub enum WatcherEvent {
    FileAdded(PathBuf),
    FileRemoved(PathBuf),
    FileModified(PathBuf),
}

/// 文件夹监控器
/// 使用 notify 库监控文件变化，通过 channel 桥接到 tokio
pub struct FolderWatcher {
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
    event_rx: mpsc::Receiver<WatcherEvent>,
}

impl FolderWatcher {
    /// 创建新的监控器
    pub fn new(path: &PathBuf) -> Result<Self> {
        // Channel：notify 是同步库，需要桥接到 tokio
        // 容量 100：防止事件积压导致 notify 回调阻塞
        let (event_tx, event_rx) = mpsc::channel(100);

        // 创建 watcher
        let config = Config::default()
            .with_poll_interval(std::time::Duration::from_secs(2));

        // 创建 event handler
        let handler = move |event: Result<Event, notify::Error>| {
            if let Ok(event) = event {
                let events = convert_notify_event(event);

                for e in events {
                    // blocking_send 不会阻塞 tokio runtime
                    if let Err(err) = event_tx.blocking_send(e) {
                        tracing::warn!("Failed to send watcher event: {}", err);
                    }
                }
            }
        };

        let mut watcher: RecommendedWatcher = Watcher::new(handler, config)?;

        // 开始监控
        watcher.watch(path, RecursiveMode::Recursive)?;

        tracing::info!("Started watching directory: {:?}", path);

        Ok(Self { watcher, event_rx })
    }

    /// 异步等待下一个事件
    pub async fn next_event(&mut self) -> Option<WatcherEvent> {
        self.event_rx.recv().await
    }

    /// 停止监控
    pub fn stop(&mut self) {
        tracing::info!("Stopped watching directory");
    }
}

/// 转换 notify 事件到内部事件类型
fn convert_notify_event(event: Event) -> Vec<WatcherEvent> {
    let mut events = Vec::new();

    for path in event.paths {
        if event.kind.is_create() {
            events.push(WatcherEvent::FileAdded(path));
        } else if event.kind.is_remove() {
            events.push(WatcherEvent::FileRemoved(path));
        } else if event.kind.is_modify() {
            events.push(WatcherEvent::FileModified(path));
        }
    }

    events
}