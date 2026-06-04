# Lan Media Hub

专业级局域网媒体共享桌面软件，基于 Rust (Tauri v2) + Vue 3 构建。

## ✨ 功能特性

- 📁 **文件共享** - 轻松共享本地文件夹，支持局域网内多设备访问
- 🎬 **视频播放** - 支持主流视频格式，FFmpeg 实时转码
- 🖼️ **图片浏览** - 支持常见图片格式预览
- 🎵 **音频播放** - 支持多种音频格式
- 🔒 **访问控制** - 可选密码保护，JWT 认证
- 📱 **移动适配** - 响应式 Web UI，支持手机访问
- 🔍 **文件搜索** - 快速搜索文件名
- 📊 **统计信息** - 文件数量、大小等统计

## 🚀 快速开始

### 环境要求

- Rust 1.75+
- Node.js 18+
- FFmpeg（用于视频转码和缩略图生成）

### 安装

```bash
# 克隆项目
git clone https://github.com/yourusername/lan-media-hub.git
cd lan-media-hub

# 安装前端依赖
cd frontend && npm install && cd ..
cd web-ui && npm install && cd ..

# 开发模式运行
cargo tauri dev
```

### 构建

```bash
# 构建生产版本
cargo tauri build
```

## ⚙️ 配置

### 配置文件位置

- **Windows**: `%LOCALAPPDATA%\LanMediaHub\settings.json`
- **macOS/Linux**: `~/.config/LanMediaHub/settings.json`

### 配置项说明

```json
{
  "port": 8241,
  "host": "0.0.0.0",
  "db_path": "lan_media_hub_v2.db",
  "auto_start": true,
  "password": ""
}
```

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `port` | u16 | 8241 | HTTP 服务器端口 |
| `host` | string | "0.0.0.0" | 绑定地址 |
| `db_path` | string | "lan_media_hub_v2.db" | 数据库文件名 |
| `auto_start` | bool | true | 启动时自动开启服务器 |
| `password` | string | "" | 访问密码（为空则无需认证） |

### 数据库

数据库文件位于配置目录下：`lan_media_hub_v2.db`（SQLite 格式）

包含以下表：
- `share_configs` - 共享文件夹配置
- `media_index` - 媒体文件索引
- `kv_store` - 键值存储（密码、JWT 密钥等）

### 缩略图缓存

缩略图缓存位于：`%LOCALAPPDATA%\LanMediaHub\thumbnails\`

转码缓存位于：`%LOCALAPPDATA%\LanMediaHub\transcode\`

## 🔧 故障排除

### FFmpeg 相关问题

**问题**: 缩略图生成失败或视频转码不工作

**解决方案**:
1. 确保 FFmpeg 已安装并添加到 PATH 环境变量
2. 在命令行运行 `ffmpeg -version` 验证安装
3. Windows 用户可以从 https://ffmpeg.org/download.html 下载

### 端口被占用

**问题**: 启动时报错 "Address already in use"

**解决方案**:
1. 修改 `settings.json` 中的 `port` 值
2. 或者关闭占用端口的程序：
   - Windows: `netstat -ano | findstr :8241`
   - macOS/Linux: `lsof -i :8241`

### 无法被其他设备访问

**问题**: 手机或其他设备无法连接

**解决方案**:
1. 确保所有设备在同一局域网
2. 检查防火墙设置，允许 8241 端口
3. 尝试使用 `http://你的IP:8241` 访问

### 文件监控不工作

**问题**: 添加新文件后没有自动索引

**解决方案**:
1. 检查日志输出查看是否有错误
2. 尝试手动刷新或重启应用
3. 某些网络驱动器可能不支持文件监控

### 数据库损坏

**问题**: 应用启动失败，日志显示数据库错误

**解决方案**:
1. 备份并删除 `lan_media_hub_v2.db` 文件
2. 重启应用会自动创建新数据库
3. 重新添加共享文件夹

## 📦 项目结构

```
lan-media-hub/
├── crates/
│   ├── config/          # 配置管理
│   ├── core/            # 核心业务逻辑
│   │   ├── db/          # 数据库层（SQLite + sqlx）
│   │   ├── index/       # 文件索引和扫描
│   │   └── share/       # 共享管理
│   └── http/            # HTTP 服务器（Axum）
│       ├── routes/      # API 路由
│       └── auth/        # JWT 认证
├── src-tauri/           # Tauri 桌面应用入口
├── frontend/            # Tauri 桌面应用前端（Vue 3）
└── web-ui/              # 独立 Web UI（Vue 3）
```

## 🛠️ 技术栈

- **后端**: Rust, Axum, SQLite, sqlx, FFmpeg
- **前端**: Vue 3, Pinia, Tailwind CSS, Vite
- **桌面**: Tauri v2
- **认证**: JWT (jsonwebtoken)

## 📝 API 接口

### 公开接口（无需认证）

- `GET /api/auth?pw=...` - 验证密码
- `POST /api/login?pw=...` - 获取 JWT Token

### 需认证接口（Bearer Token）

- `GET /api/shares` - 获取共享列表
- `GET /api/shares/:id` - 获取共享详情
- `GET /api/browse/*rest` - 浏览文件
- `GET /api/stream/*rest` - 文件下载
- `GET /api/transcode/*rest` - 视频转码
- `GET /api/thumbnail/*rest` - 获取缩略图
- `GET /api/info/*rest` - 获取文件信息
- `POST /api/upload/*rest` - 上传文件

## 📄 许可证

MIT License
