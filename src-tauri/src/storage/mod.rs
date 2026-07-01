use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::errors::{AppError, AppResult};
use crate::models::{ConfigPreset, MergeRouteConfig, RepositoryEntry, WorkingCopyEntry};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub svn_bin: Option<String>,
    #[serde(default)]
    pub repositories: Vec<RepositoryEntry>,
    #[serde(default)]
    pub working_copies: Vec<WorkingCopyEntry>,
    #[serde(default)]
    pub merge_route_configs: Vec<MergeRouteConfig>,
    #[serde(default)]
    pub config_presets: Vec<ConfigPreset>,
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
            .unwrap_or_else(default_svn_bin)
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

/// 未手动配置时自动定位 SVN，解决 macOS GUI 应用不继承终端 PATH 的问题。
fn default_svn_bin() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Some(path) = find_macos_svn() {
            return path.to_string_lossy().into_owned();
        }
    }

    "svn".into()
}

#[cfg(target_os = "macos")]
fn find_macos_svn() -> Option<PathBuf> {
    // 先尊重启动环境中的 PATH，再覆盖 Homebrew、MacPorts 和系统常见安装位置。
    if let Some(paths) = std::env::var_os("PATH") {
        if let Some(path) = std::env::split_paths(&paths)
            .map(|dir| dir.join("svn"))
            .find(|path| path.is_file())
        {
            return Some(path);
        }
    }

    [
        "/opt/homebrew/bin/svn",
        "/opt/homebrew/opt/subversion/bin/svn",
        "/usr/local/bin/svn",
        "/usr/local/opt/subversion/bin/svn",
        "/opt/local/bin/svn",
        "/usr/bin/svn",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.is_file())
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
    let mut config = if path.exists() {
        let txt = fs::read_to_string(&path)?;
        serde_json::from_str(&txt).unwrap_or_default()
    } else {
        AppConfig::default()
    };
    let migrated = migrate_legacy_working_copy_paths(&mut config);
    if migrated {
        // 旧版本可能保存了相对于根目录的 Volumes/...，开发模式下会相对于源码目录解析。
        let json = serde_json::to_string_pretty(&config)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, json)?;
    }
    Ok(ConfigState {
        config: Mutex::new(config),
        config_path: path,
    })
}

/// 将旧配置中可从系统根目录定位的相对路径迁移为绝对路径。
fn migrate_legacy_working_copy_paths(config: &mut AppConfig) -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }

    let mut changed = false;
    for working_copy in &mut config.working_copies {
        let stored = std::path::Path::new(&working_copy.path);
        if stored.is_absolute() {
            continue;
        }

        let absolute = std::path::Path::new("/").join(stored);
        if absolute.is_dir() {
            working_copy.path = absolute.to_string_lossy().into_owned();
            changed = true;
        }
    }
    changed
}
