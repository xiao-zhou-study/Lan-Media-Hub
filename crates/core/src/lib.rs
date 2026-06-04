pub mod share;
pub mod index;
pub mod db;

pub use share::{SharedFolder, SharedFolderManager, ShareConfig, ShareStatus};
pub use index::{MediaIndex, MediaItem, IndexScanner, FolderWatcher, WatcherEvent, IndexStats};

/// 支持的视频扩展名
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm",
    "mpg", "mpeg", "ts", "mts", "m2ts", "vob",
    "rm", "rmvb", "3gp", "asf", "divx", "ogv", "m4v",
];

/// 支持的音频扩展名
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "aac", "ogg", "m4a", "wma",
];

/// 支持的图片扩展名
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff",
];

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
        if VIDEO_EXTENSIONS.contains(&ext.as_str()) { MediaType::Video }
        else if AUDIO_EXTENSIONS.contains(&ext.as_str()) { MediaType::Audio }
        else if IMAGE_EXTENSIONS.contains(&ext.as_str()) { MediaType::Image }
        else { MediaType::Other }
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

/// 根据扩展名分类媒体类型
pub fn classify_extension(ext: &str) -> &'static str {
    let ext = ext.to_lowercase();
    if VIDEO_EXTENSIONS.contains(&ext.as_str()) { "video" }
    else if AUDIO_EXTENSIONS.contains(&ext.as_str()) { "audio" }
    else if IMAGE_EXTENSIONS.contains(&ext.as_str()) { "image" }
    else { "file" }
}

/// 根据扩展名获取 MIME 类型
pub fn mime_for_extension(ext: &str) -> &'static str {
    let ext = ext.to_lowercase();
    match ext.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "avi" | "divx" => "video/x-msvideo",
        "mov" => "video/quicktime",
        "mpg" | "mpeg" => "video/mpeg",
        "wmv" | "asf" => "video/x-ms-wmv",
        "flv" => "video/x-flv",
        "ogv" | "ogg" => "video/ogg",
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        "aac" => "audio/aac",
        "m4a" | "wma" => "audio/mp4",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

/// 解析文件的真实扩展名（处理 .bc! 等未完成下载后缀）
pub fn resolve_real_extension(path: &std::path::Path) -> String {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if ext == "bc!" {
        path.with_extension("")
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default()
    } else {
        ext
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_extensions() {
        assert_eq!(MediaType::from_extension("mp4"), MediaType::Video);
        assert_eq!(MediaType::from_extension("MKV"), MediaType::Video);
        assert_eq!(MediaType::from_extension("avi"), MediaType::Video);
        assert_eq!(MediaType::from_extension("mov"), MediaType::Video);
        assert_eq!(MediaType::from_extension("webm"), MediaType::Video);
        assert_eq!(MediaType::from_extension("flv"), MediaType::Video);
        assert_eq!(MediaType::from_extension("wmv"), MediaType::Video);
        assert_eq!(MediaType::from_extension("mpg"), MediaType::Video);
        assert_eq!(MediaType::from_extension("ts"), MediaType::Video);
        assert_eq!(MediaType::from_extension("m4v"), MediaType::Video);
    }

    #[test]
    fn test_audio_extensions() {
        assert_eq!(MediaType::from_extension("mp3"), MediaType::Audio);
        assert_eq!(MediaType::from_extension("flac"), MediaType::Audio);
        assert_eq!(MediaType::from_extension("wav"), MediaType::Audio);
        assert_eq!(MediaType::from_extension("aac"), MediaType::Audio);
        assert_eq!(MediaType::from_extension("ogg"), MediaType::Audio);
        assert_eq!(MediaType::from_extension("m4a"), MediaType::Audio);
        assert_eq!(MediaType::from_extension("wma"), MediaType::Audio);
    }

    #[test]
    fn test_image_extensions() {
        assert_eq!(MediaType::from_extension("jpg"), MediaType::Image);
        assert_eq!(MediaType::from_extension("jpeg"), MediaType::Image);
        assert_eq!(MediaType::from_extension("png"), MediaType::Image);
        assert_eq!(MediaType::from_extension("gif"), MediaType::Image);
        assert_eq!(MediaType::from_extension("bmp"), MediaType::Image);
        assert_eq!(MediaType::from_extension("webp"), MediaType::Image);
        assert_eq!(MediaType::from_extension("tiff"), MediaType::Image);
    }

    #[test]
    fn test_other_extensions() {
        assert_eq!(MediaType::from_extension("txt"), MediaType::Other);
        assert_eq!(MediaType::from_extension("pdf"), MediaType::Other);
        assert_eq!(MediaType::from_extension(""), MediaType::Other);
        assert_eq!(MediaType::from_extension("xyz"), MediaType::Other);
    }

    #[test]
    fn test_case_insensitive() {
        assert_eq!(MediaType::from_extension("MP4"), MediaType::Video);
        assert_eq!(MediaType::from_extension("Mp3"), MediaType::Audio);
        assert_eq!(MediaType::from_extension("JPG"), MediaType::Image);
    }

    #[test]
    fn test_classify_extension() {
        assert_eq!(classify_extension("mp4"), "video");
        assert_eq!(classify_extension("mp3"), "audio");
        assert_eq!(classify_extension("jpg"), "image");
        assert_eq!(classify_extension("txt"), "file");
        assert_eq!(classify_extension("MP4"), "video");
    }

    #[test]
    fn test_mime_for_extension() {
        assert_eq!(mime_for_extension("mp4"), "video/mp4");
        assert_eq!(mime_for_extension("mkv"), "video/x-matroska");
        assert_eq!(mime_for_extension("webm"), "video/webm");
        assert_eq!(mime_for_extension("mp3"), "audio/mpeg");
        assert_eq!(mime_for_extension("flac"), "audio/flac");
        assert_eq!(mime_for_extension("jpg"), "image/jpeg");
        assert_eq!(mime_for_extension("png"), "image/png");
        assert_eq!(mime_for_extension("unknown"), "application/octet-stream");
    }

    #[test]
    fn test_resolve_real_extension() {
        use std::path::Path;

        assert_eq!(resolve_real_extension(Path::new("video.mp4")), "mp4");
        assert_eq!(resolve_real_extension(Path::new("song.mp3")), "mp3");

        assert_eq!(resolve_real_extension(Path::new("video.mp4.bc!")), "mp4");
        assert_eq!(resolve_real_extension(Path::new("movie.mkv.bc!")), "mkv");

        assert_eq!(resolve_real_extension(Path::new("noext")), "");
        assert_eq!(resolve_real_extension(Path::new("noext.bc!")), "");
    }
}