use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::errors::{AppError, AppResult};
use crate::models::{RepositoryEntry, WorkingCopyEntry};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub svn_bin: Option<String>,
    #[serde(default)]
    pub repositories: Vec<RepositoryEntry>,
    #[serde(default)]
    pub working_copies: Vec<WorkingCopyEntry>,
}

pub struct ConfigState {
    pub config: Mutex<AppConfig>,
    pub config_path: PathBuf,
}

impl ConfigState {
    pub fn svn_bin(&self) -> String {
        self.config
            .lock()
            .ok()
            .and_then(|c| c.svn_bin.clone())
            .unwrap_or_else(|| "svn".into())
    }

    pub fn save(&self) -> AppResult<()> {
        let cfg = self
            .config
            .lock()
            .map_err(|_| AppError::Other("config 锁被污染".into()))?
            .clone();
        let json = serde_json::to_string_pretty(&cfg)?;
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.config_path, json)?;
        Ok(())
    }
}

fn config_file_path(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Other(format!("获取 app_config_dir 失败: {}", e)))?;
    Ok(dir.join("config.json"))
}

pub fn init_config_state(app: &AppHandle) -> AppResult<ConfigState> {
    let path = config_file_path(app)?;
    let config = if path.exists() {
        let txt = fs::read_to_string(&path)?;
        serde_json::from_str(&txt).unwrap_or_default()
    } else {
        AppConfig::default()
    };
    Ok(ConfigState {
        config: Mutex::new(config),
        config_path: path,
    })
}
