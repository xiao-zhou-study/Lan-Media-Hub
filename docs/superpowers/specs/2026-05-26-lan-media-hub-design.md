---
name: lan-media-hub-design
description: 局域网媒体共享桌面软件第一阶段 MVP 设计
---

# Lan Media Hub - 第一阶段 MVP 设计

## 项目定位

混合型局域网媒体共享软件（类似 Jellyfin + AirDrop + NAS），桌面端提供服务，手机浏览器访问消费内容。

## 已确认决策

| 决策项 | 选择 |
|--------|------|
| 第一阶段功能 | 文件夹共享 + HTTP 服务 + Range 流媒体 |
| 桌面 UI | 系统托盘为主，简单设置面板 |
| SQLite 存储 | 共享配置 + 基础媒体索引 |
| 媒体处理 | 基础索引（无 FFmpeg），后续迭代 |
| 并发场景 | 单设备访问 |
| 架构模式 | 嵌入式 HTTP，共享 Tokio Runtime |

## 技术栈

- Tauri v2 + Rust + Tokio + Axum + SQLite
- Vue3 + Pinia + Tailwind + TypeScript
- notify（文件监控）

## 模块划分

```
lan-media-hub/
├── src-tauri/          # Tauri 入口，main.rs < 50 行
├── crates/
│   ├── core/           # 业务逻辑（无 Tauri 依赖）
│   ├── http/           # Axum HTTP 服务
│   └── config/         # 配置管理
├── frontend/           # Vue3 前端
```

## 状态共享

- `Arc<RwLock<MediaIndex>>`：高频读取，低频写入
- `Arc<RwLock<SharedFolderManager>>`：共享文件夹管理
- SQLite 通过 sqlx 异步访问

## 关键实现要点

1. **Range Requests**：206 Partial Content，iOS Safari 兼容
2. **Notify Watcher**：mpsc channel 桥接到 tokio，增量索引
3. **路径安全**：禁止路径遍历，校验共享边界
4. **流式传输**：不一次性读取大文件，tokio::fs::File

## 后续阶段（不在 MVP）

- mDNS 自动发现
- FFmpeg 元数据提取 + 缩略图
- Token 鉴权
- 播放历史/进度