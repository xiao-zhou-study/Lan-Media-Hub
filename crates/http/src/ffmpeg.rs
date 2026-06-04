use std::path::PathBuf;

pub fn ffmpeg_path() -> String {
    ffmpeg_sidecar::paths::ffmpeg_path().to_string_lossy().to_string()
}

pub fn ffprobe_path() -> String {
    let ffmpeg = ffmpeg_sidecar::paths::ffmpeg_path();
    ffmpeg.with_file_name(if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" })
        .to_string_lossy().to_string()
}

/// ffprobe 检测源视频编码
pub async fn probe_video_codec(path: &PathBuf) -> String {
    match tokio::process::Command::new(ffprobe_path())
        .arg("-v").arg("error")
        .arg("-select_streams").arg("v:0")
        .arg("-show_entries").arg("stream=codec_name")
        .arg("-of").arg("csv=p=0")
        .arg(path.to_string_lossy().to_string())
        .output().await
    {
        Ok(o) => String::from_utf8(o.stdout).unwrap_or_default().trim().to_string(),
        Err(_) => String::new(),
    }
}
