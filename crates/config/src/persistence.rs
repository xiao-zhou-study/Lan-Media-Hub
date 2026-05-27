use crate::Settings;
use anyhow::{Context, Result};
use std::path::Path;
use tokio::fs;

pub struct SettingsPersistence {
    file_path: std::path::PathBuf,
}

impl SettingsPersistence {
    pub fn new(app_data_dir: &Path) -> Self {
        let file_path = app_data_dir.join(crate::SETTINGS_FILE_NAME);
        Self { file_path }
    }

    pub async fn load(&self) -> Result<Settings> {
        if !self.file_path.exists() {
            tracing::info!("Settings file not found, using defaults");
            return Ok(Settings::default());
        }

        let content = fs::read_to_string(&self.file_path)
            .await
            .context("Failed to read settings file")?;

        let settings: Settings = serde_json::from_str(&content)
            .context("Failed to parse settings JSON")?;

        tracing::info!("Loaded settings from {:?}", self.file_path);
        Ok(settings)
    }

    pub async fn save(&self, settings: &Settings) -> Result<()> {
        // 确保目录存在
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create settings directory")?;
        }

        let content = serde_json::to_string_pretty(settings)
            .context("Failed to serialize settings")?;

        fs::write(&self.file_path, content)
            .await
            .context("Failed to write settings file")?;

        tracing::info!("Saved settings to {:?}", self.file_path);
        Ok(())
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
}