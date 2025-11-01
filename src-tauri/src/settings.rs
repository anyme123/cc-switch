use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

/// 应用设置结构，允许覆盖默认配置目录
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default = "default_show_in_tray")]
    pub show_in_tray: bool,
    #[serde(default = "default_minimize_to_tray_on_close")]
    pub minimize_to_tray_on_close: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_config_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_config_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub droid_config_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global_shortcut: Option<String>,
    #[serde(default)]
    pub auto_refresh_balance: bool,
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval: u32,
    #[serde(default = "default_refresh_only_when_visible")]
    pub refresh_only_when_visible: bool,
}

fn default_show_in_tray() -> bool {
    true
}

fn default_minimize_to_tray_on_close() -> bool {
    true
}

fn default_refresh_interval() -> u32 {
    300 // 5分钟（单位：秒）
}

fn default_refresh_only_when_visible() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            show_in_tray: true,
            minimize_to_tray_on_close: true,
            claude_config_dir: None,
            codex_config_dir: None,
            droid_config_dir: None,
            language: None,
            auto_start: false,
            global_shortcut: None,
            auto_refresh_balance: false,
            refresh_interval: 300,
            refresh_only_when_visible: true,
        }
    }
}

impl AppSettings {
    fn settings_path() -> PathBuf {
        crate::config::get_app_config_dir().join("settings.json")
    }

    fn normalize_paths(&mut self) {
        self.claude_config_dir = self
            .claude_config_dir
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        self.codex_config_dir = self
            .codex_config_dir
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        self.droid_config_dir = self
            .droid_config_dir
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        self.language = self
            .language
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| matches!(*s, "en" | "zh"))
            .map(|s| s.to_string());

        self.global_shortcut = self
            .global_shortcut
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        // 限制刷新间隔在合理范围内（1分钟到30分钟）
        if self.refresh_interval < 60 {
            self.refresh_interval = 60;
        } else if self.refresh_interval > 1800 {
            self.refresh_interval = 1800;
        }
    }

    pub fn load() -> Self {
        let path = Self::settings_path();
        if let Ok(content) = fs::read_to_string(&path) {
            match serde_json::from_str::<AppSettings>(&content) {
                Ok(mut settings) => {
                    settings.normalize_paths();
                    settings
                }
                Err(err) => {
                    log::warn!(
                        "解析设置文件失败，将使用默认设置。路径: {}, 错误: {}",
                        path.display(),
                        err
                    );
                    Self::default()
                }
            }
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let mut normalized = self.clone();
        normalized.normalize_paths();
        let path = Self::settings_path();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建设置目录失败: {}", e))?;
        }

        let json = serde_json::to_string_pretty(&normalized)
            .map_err(|e| format!("序列化设置失败: {}", e))?;
        fs::write(&path, json).map_err(|e| format!("写入设置失败: {}", e))?;
        Ok(())
    }
}

fn settings_store() -> &'static RwLock<AppSettings> {
    static STORE: OnceLock<RwLock<AppSettings>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(AppSettings::load()))
}

fn resolve_override_path(raw: &str) -> PathBuf {
    if raw == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    } else if let Some(stripped) = raw.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    } else if let Some(stripped) = raw.strip_prefix("~\\") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }

    PathBuf::from(raw)
}

pub fn get_settings() -> AppSettings {
    settings_store().read().expect("读取设置锁失败").clone()
}

pub fn update_settings(mut new_settings: AppSettings) -> Result<(), String> {
    new_settings.normalize_paths();
    new_settings.save()?;

    let mut guard = settings_store().write().expect("写入设置锁失败");
    *guard = new_settings;
    Ok(())
}

pub fn get_claude_override_dir() -> Option<PathBuf> {
    let settings = settings_store().read().ok()?;
    settings
        .claude_config_dir
        .as_ref()
        .map(|p| resolve_override_path(p))
}

pub fn get_codex_override_dir() -> Option<PathBuf> {
    let settings = settings_store().read().ok()?;
    settings
        .codex_config_dir
        .as_ref()
        .map(|p| resolve_override_path(p))
}

pub fn get_droid_override_dir() -> Option<PathBuf> {
    let settings = settings_store().read().ok()?;
    settings
        .droid_config_dir
        .as_ref()
        .map(|p| resolve_override_path(p))
}
