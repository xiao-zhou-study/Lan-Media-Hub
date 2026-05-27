pub mod settings;
pub mod persistence;

pub use settings::Settings;
pub use persistence::SettingsPersistence;

pub const DEFAULT_PORT: u16 = 8241;
pub const DEFAULT_DB_NAME: &str = "lan_media_hub_v2.db";
pub const SETTINGS_FILE_NAME: &str = "settings.json";