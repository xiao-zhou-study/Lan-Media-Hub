# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Lan Media Hub — a professional LAN media sharing desktop app built with Rust (Tauri v2) + Axum + SQLite + Vue 3 frontends.

## Build & Run

```bash
cargo build                          # Build Rust workspace
cargo tauri dev                      # Run Tauri desktop app (dev)
cargo tauri build                    # Production Tauri build
cargo test -p lan-media-hub-core     # Run tests for specific crate
cargo fmt --check                    # Format check
cargo clippy -- -D warnings          # Lint

# Frontends
cd frontend && npm run dev           # Tauri desktop app dev (port 5200)
cd frontend && npm run build         # → frontend/dist/
cd web-ui && npm run dev             # Standalone web UI dev (port 8242)
cd web-ui && npm run build           # → web-ui/dist/ (served by HTTP server at /)
```

## Architecture

### Rust Workspace (4 crates)

```
crates/config/    — Settings struct, LAN IP detection (get_if_addrs)
crates/core/      — SharedFolderManager, MediaIndex, DB layer, scanner, notify watcher
crates/http/      — Axum server, REST routes, streaming, auth (JWT), thumbnails
src-tauri/        — Tauri v2 entry, commands.rs, AppState, tray
```

**Depends:** `src-tauri` → `http` + `core` + `config`. No reverse deps.

### Startup Flow

1. `main.rs` opens SQLite → loads `settings.json` → creates `AppState`
2. Restores shares/index from DB via `SharedFolderManager::load_from_db` / `MediaIndex::load_from_db`
3. Auto-spawns `HttpServer::start()` in a Tauri async task
4. Tauri commands read/write `Arc<RwLock<AppState>>`

### State (`AppState`)

All shareable state wrapped in `Arc<RwLock<T>>` (tokio async RwLock, not std):

| Field | Purpose |
|---|---|
| `shared_folders` | SharedFolderManager — share lifecycle |
| `media_index` | MediaIndex — in-memory file index |
| `settings` | Settings — port, host, password |
| `password` | `Arc<RwLock<String>>` — shared with HTTP server for dynamic auth |
| `jwt_secret` | `Arc<RwLock<String>>` — JWT signing key |
| `server_running` | `Arc<RwLock<bool>>` |
| `db` | `Option<Arc<Database>>` |

### HTTP API (`/api`)

**Public routes** (no auth):
- `GET /api/auth?pw=...` — verify password
- `POST /api/login?pw=...` — get JWT token

**Protected routes** (JWT required when password set):
- `GET /api/shares`, `GET /api/shares/:id`
- `GET /api/browse/*rest` — file listing with `?sort=name|size|modified&order=asc|desc`
- `GET /api/stream/*rest` — file download with Range support
- `GET /api/transcode/*rest?start=N` — FFmpeg real-time MP4 transcode
- `GET /api/thumbnail/*rest?size=N` — video frame / image thumbnail
- `GET /api/info/*rest` — ffprobe metadata (duration, resolution)
- `POST /api/upload/*rest` — multipart upload

Auth is implemented via an `Auth` extractor (`FromRequestParts<S>`) in `crates/http/src/auth.rs`. Handlers add `_auth: Auth` parameter. No password = auth skipped.

### Database (SQLite via sqlx)

Schema in `crates/core/src/db/schema.rs`. Individual `CREATE TABLE IF NOT EXISTS` statements (sqlx doesn't support multi-statement batches).

| Table | Key columns |
|---|---|
| `share_configs` | id, path (UNIQUE), name, created_at, status |
| `media_index` | id, share_id (FK), name, full_path (UNIQUE), size, media_type, extension, modified_at, indexed_at |
| `kv_store` | key (PK), value |

**Performance:** Batch inserts use transactions (`batch_insert_media_items`) — 10-50x faster than per-row commits. Stats loaded on startup via `COUNT(*) + SUM(size)` query.

### File Streaming

`stream_file()` in `routes/share.rs` handles HTTP Range requests. Parses `Range: bytes=N-M` → 206 Partial Content with byte range, or 200 with `ReaderStream` for full download.

### FFmpeg Integration

- **Thumbnails:** `ffmpeg -ss 3 -vframes 1 -q:v 2` → cached JPEG in `%LOCALAPPDATA%/LanMediaHub/thumbnails/`
- **Transcoding:** `ffmpeg -ss N -i input -f mp4 pipe:1` → real-time H.264 stream for non-MP4 formats
- **Info:** `ffprobe -show_format -show_streams` → duration/resolution/codec metadata

### Path Safety

All file access: `path_clean::PathClean::clean()` → `starts_with()` check against share root. Prevents directory traversal.

### Frontend Duality

- **`frontend/`** — Tauri desktop app. Vue 3 + Pinia + Tailwind 3. Uses `@tauri-apps/api` for IPC. Dev port 5200.
- **`web-ui/`** — Standalone SPA served by HTTP server at `/`. Vue 3 + Vue Router + Tailwind 3. Dev port 8242. Built output in `web-ui/dist/` — served by `ServeDir` fallback to `index.html` for SPA routing.

### Settings Persistence

- `settings.json` in `%LOCALAPPDATA%/LanMediaHub/` — password, port (loaded on startup, saved on change)
- `lan_media_hub.db` in same directory — shares, media index, kv_store
- JWT secret regenerated on startup (not persisted — sessions expire after restart)

## Key Conventions

- Tauri commands are async, return `Result<T, String>`
- UUIDs as share IDs, passed as strings across FFI/HTTP boundaries
- `path-clean` + share boundary check before any file access
- `walkdir` in `tokio::task::spawn_blocking` for directory scanning
- Notify watcher uses `mpsc::channel` (sync → async bridge) for file change events