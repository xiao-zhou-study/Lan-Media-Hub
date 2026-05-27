pub mod share;
pub mod index;
pub mod db;

pub use share::{SharedFolder, SharedFolderManager, ShareConfig, ShareStatus};
pub use index::{MediaIndex, MediaItem, IndexScanner, FolderWatcher, WatcherEvent, IndexStats};

/// 支持的媒体类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MediaType {
    Video,
    Audio,
    Image,
    Other,
}

impl MediaType {
    pub fn from_extension(ext: &str) -> Self {
        let ext = ext.to_lowercase();
        match ext.as_str() {
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" => MediaType::Video,
            "mp3" | "flac" | "wav" | "aac" | "ogg" | "m4a" | "wma" => MediaType::Audio,
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tiff" => MediaType::Image,
            _ => MediaType::Other,
        }
    }

    pub fn mime_type(&self) -> &'static str {
        match self {
            MediaType::Video => "video/mp4",
            MediaType::Audio => "audio/mpeg",
            MediaType::Image => "image/jpeg",
            MediaType::Other => "application/octet-stream",
        }
    }
}