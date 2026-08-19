use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_pet")]
    pub pet: String,
    #[serde(default)]
    pub position: Position,
    #[serde(default = "default_scale")]
    pub scale: f64,
    #[serde(default = "default_true")]
    pub always_on_top: bool,
    /// Occasional random one-shot animations while the pet is idle.
    #[serde(default = "default_false")]
    pub idle_animation: bool,
    /// Loop the idle clip instead of freezing on its first frame.
    /// Settings UI can toggle this later; missing configs default to looping.
    #[serde(default = "default_true")]
    pub loop_idle: bool,
    /// Pointer awareness ring in physical pixels. Default matches the original 240px ring.
    #[serde(default = "default_look_range")]
    pub look_range: u32,
    /// Display scale from the settings window. Defaults to 1.0 so existing
    /// unused `scale` values (0.75) do not shrink the pet.
    #[serde(default = "default_display_scale")]
    pub display_scale: f64,
    /// Language setting: "auto" (system default), "zh", or "en".
    #[serde(default = "default_lang")]
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

fn default_pet() -> String {
    String::new()
}
fn default_scale() -> f64 {
    0.75
}
fn default_look_range() -> u32 {
    240
}
fn default_display_scale() -> f64 {
    1.0
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_lang() -> String {
    "auto".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pet: default_pet(),
            position: Position::default(),
            scale: default_scale(),
            always_on_top: true,
            idle_animation: false,
            loop_idle: true,
            look_range: default_look_range(),
            display_scale: default_display_scale(),
            language: default_lang(),
        }
    }
}

impl Config {
    pub fn config_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "CodingPet")
            .map(|dirs| dirs.config_dir().join("config.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let Some(path) = Self::config_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("Failed to create config dir: {e}");
                return;
            }
        }
        match serde_json::to_string_pretty(self) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&path, content) {
                    tracing::warn!("Failed to write config: {e}");
                }
            }
            Err(e) => tracing::warn!("Failed to serialize config: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.scale, 0.75);
        assert!(config.always_on_top);
        assert!(!config.idle_animation);
        assert!(config.loop_idle);
        assert_eq!(config.look_range, 240);
        assert_eq!(config.display_scale, 1.0);
    }

    #[test]
    fn test_config_roundtrip() {
        let config = Config {
            pet: "test:cat".to_string(),
            position: Position { x: 100, y: 200 },
            scale: 1.0,
            always_on_top: false,
            idle_animation: false,
            loop_idle: true,
            look_range: 240,
            display_scale: 1.0,
            language: "auto".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.pet, "test:cat");
        assert_eq!(loaded.position.x, 100);
        assert_eq!(loaded.position.y, 200);
    }

    #[test]
    fn test_config_corrupt_json() {
        let result: Result<Config, _> = serde_json::from_str("not json!!!");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_missing_fields_uses_defaults() {
        let json = r#"{"pet": "cat"}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.scale, 0.75);
        assert!(config.always_on_top);
        assert!(config.loop_idle);
        assert_eq!(config.look_range, 240);
        assert_eq!(config.display_scale, 1.0);
    }
}
