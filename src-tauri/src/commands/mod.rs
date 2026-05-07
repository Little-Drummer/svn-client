use chrono::Utc;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::{SvnInfo, SvnLogEntry, SvnStatusEntry, WorkingCopyEntry};
use crate::process::spawn_svn_task;
use crate::storage::ConfigState;
use crate::svn::diff::{svn_cat_base, svn_diff_file, svn_diff_revision};
use crate::svn::info::svn_info;
use crate::svn::log::{svn_log, LogOptions};
use crate::svn::runner::{check_svn_version, run_svn};
use crate::svn::status::svn_status;

// ---------- 环境检查 ----------

#[tauri::command]
pub fn svn_check_environment(state: State<ConfigState>) -> AppResult<String> {
    let bin = state.svn_bin();
    check_svn_version(&bin)
}

#[tauri::command]
pub fn get_svn_bin(state: State<ConfigState>) -> String {
    state.svn_bin()
}

#[tauri::command]
pub fn set_svn_bin(state: State<ConfigState>, bin: Option<String>) -> AppResult<()> {
    {
        let mut cfg = state
            .config
            .lock()
            .map_err(|_| AppError::Other("config 锁被污染".into()))?;
        cfg.svn_bin = bin.filter(|s| !s.trim().is_empty());
    }
    state.save()
}

// ---------- 工作副本管理 ----------

#[tauri::command]
pub fn list_working_copies(state: State<ConfigState>) -> AppResult<Vec<WorkingCopyEntry>> {
    let cfg = state
        .config
        .lock()
        .map_err(|_| AppError::Other("config 锁被污染".into()))?;
    Ok(cfg.working_copies.clone())
}

#[tauri::command]
pub fn add_working_copy(
    state: State<ConfigState>,
    path: String,
) -> AppResult<WorkingCopyEntry> {
    if path.trim().is_empty() {
        return Err(AppError::InvalidPath(path));
    }
    let bin = state.svn_bin();
    // 校验是 svn working copy
    let info = svn_info(&bin, &path).map_err(|e| match e {
        AppError::SvnCommand { stderr, .. } if stderr.to_lowercase().contains("not a working copy") => {
            AppError::NotWorkingCopy(path.clone())
        }
        other => other,
    })?;

    let entry = WorkingCopyEntry {
        id: Uuid::new_v4().to_string(),
        path: info.path.clone(),
        url: Some(info.url),
        repository_root: Some(info.repository_root),
        revision: Some(info.revision),
        last_seen_at: Some(Utc::now().to_rfc3339()),
    };

    {
        let mut cfg = state
            .config
            .lock()
            .map_err(|_| AppError::Other("config 锁被污染".into()))?;
        // 路径去重
        if let Some(existing) = cfg.working_copies.iter_mut().find(|wc| wc.path == entry.path) {
            existing.url = entry.url.clone();
            existing.repository_root = entry.repository_root.clone();
            existing.revision = entry.revision;
            existing.last_seen_at = entry.last_seen_at.clone();
            let result = existing.clone();
            drop(cfg);
            state.save()?;
            return Ok(result);
        }
        cfg.working_copies.push(entry.clone());
    }
    state.save()?;
    Ok(entry)
}

#[tauri::command]
pub fn remove_working_copy(state: State<ConfigState>, id: String) -> AppResult<()> {
    {
        let mut cfg = state
            .config
            .lock()
            .map_err(|_| AppError::Other("config 锁被污染".into()))?;
        cfg.working_copies.retain(|wc| wc.id != id);
    }
    state.save()
}

#[tauri::command]
pub fn refresh_working_copy(
    state: State<ConfigState>,
    id: String,
) -> AppResult<WorkingCopyEntry> {
    let (path, _index) = {
        let cfg = state
            .config
            .lock()
            .map_err(|_| AppError::Other("config 锁被污染".into()))?;
        let pos = cfg
            .working_copies
            .iter()
            .position(|wc| wc.id == id)
            .ok_or_else(|| AppError::Other(format!("找不到工作副本 {}", id)))?;
        (cfg.working_copies[pos].path.clone(), pos)
    };

    let bin = state.svn_bin();
    let info = svn_info(&bin, &path)?;

    let mut cfg = state
        .config
        .lock()
        .map_err(|_| AppError::Other("config 锁被污染".into()))?;
    let entry = cfg
        .working_copies
        .iter_mut()
        .find(|wc| wc.id == id)
        .ok_or_else(|| AppError::Other(format!("找不到工作副本 {}", id)))?;
    entry.url = Some(info.url);
    entry.repository_root = Some(info.repository_root);
    entry.revision = Some(info.revision);
    entry.last_seen_at = Some(Utc::now().to_rfc3339());
    let result = entry.clone();
    drop(cfg);
    state.save()?;
    Ok(result)
}

// ---------- 只读查询 ----------

#[tauri::command]
pub fn svn_get_info(state: State<ConfigState>, path: String) -> AppResult<SvnInfo> {
    let bin = state.svn_bin();
    svn_info(&bin, &path)
}

#[tauri::command]
pub fn svn_get_status(
    state: State<ConfigState>,
    path: String,
    show_unversioned: Option<bool>,
) -> AppResult<Vec<SvnStatusEntry>> {
    let bin = state.svn_bin();
    svn_status(&bin, &path, show_unversioned.unwrap_or(true))
}

#[tauri::command]
pub fn svn_get_log(
    state: State<ConfigState>,
    path: String,
    limit: Option<u32>,
    revision_range: Option<String>,
    search: Option<String>,
    with_paths: Option<bool>,
) -> AppResult<Vec<SvnLogEntry>> {
    let bin = state.svn_bin();
    let opts = LogOptions {
        target: &path,
        limit: limit.unwrap_or(50).min(500),
        revision_range: revision_range.as_deref(),
        search: search.as_deref(),
        with_paths: with_paths.unwrap_or(true),
    };
    svn_log(&bin, &opts)
}

#[tauri::command]
pub fn svn_get_diff(state: State<ConfigState>, path: String) -> AppResult<String> {
    let bin = state.svn_bin();
    svn_diff_file(&bin, &path)
}

#[tauri::command]
pub fn svn_get_diff_revision(
    state: State<ConfigState>,
    path: String,
    revision: u64,
) -> AppResult<String> {
    let bin = state.svn_bin();
    svn_diff_revision(&bin, &path, revision)
}

#[tauri::command]
pub fn svn_get_base_content(state: State<ConfigState>, path: String) -> AppResult<String> {
    let bin = state.svn_bin();
    svn_cat_base(&bin, &path)
}

// 读取磁盘文件文本，用于 diff 视图的 split 模式（避免引入完整 fs plugin）
#[tauri::command]
pub fn read_file_text(path: String) -> AppResult<String> {
    let bytes = std::fs::read(&path).map_err(AppError::Io)?;
    Ok(crate::svn::runner::decode_output(&bytes))
}

// 简单的 revert（用于撤销本地改动），调用方负责前端二次确认
#[tauri::command]
pub fn svn_revert(state: State<ConfigState>, paths: Vec<String>) -> AppResult<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let bin = state.svn_bin();
    let mut args: Vec<&str> = vec!["revert", "--non-interactive"];
    for p in &paths {
        args.push(p);
    }
    run_svn(&bin, &args)?;
    Ok(())
}

// ---------- 长任务 ----------

#[tauri::command]
pub fn svn_start_commit(
    app: AppHandle,
    state: State<ConfigState>,
    paths: Vec<String>,
    message: String,
) -> AppResult<String> {
    if paths.is_empty() {
        return Err(AppError::Other("commit 需要至少一个文件".into()));
    }
    let bin = state.svn_bin();
    let mut args: Vec<String> = vec![
        "commit".into(),
        "--non-interactive".into(),
        "-m".into(),
        message,
    ];
    for p in paths {
        args.push(p);
    }
    spawn_svn_task(app, bin, args)
}

#[tauri::command]
pub fn svn_start_update(
    app: AppHandle,
    state: State<ConfigState>,
    path: String,
    revision: Option<String>,
) -> AppResult<String> {
    let bin = state.svn_bin();
    let mut args: Vec<String> = vec!["update".into(), "--non-interactive".into()];
    if let Some(r) = revision {
        if !r.trim().is_empty() {
            args.push("-r".into());
            args.push(r);
        }
    }
    args.push(path);
    spawn_svn_task(app, bin, args)
}

#[tauri::command]
pub fn svn_start_checkout(
    app: AppHandle,
    state: State<ConfigState>,
    url: String,
    target_path: String,
    revision: Option<String>,
    username: Option<String>,
    password: Option<String>,
) -> AppResult<String> {
    if url.trim().is_empty() || target_path.trim().is_empty() {
        return Err(AppError::Other("URL 和目标目录不能为空".into()));
    }
    let bin = state.svn_bin();
    let mut args: Vec<String> = vec!["checkout".into(), "--non-interactive".into()];
    if let Some(r) = revision {
        if !r.trim().is_empty() {
            args.push("-r".into());
            args.push(r);
        }
    }
    if let Some(u) = username {
        if !u.is_empty() {
            args.push("--username".into());
            args.push(u);
        }
    }
    if let Some(p) = password {
        if !p.is_empty() {
            args.push("--password".into());
            args.push(p);
        }
    }
    args.push(url);
    args.push(target_path);
    spawn_svn_task(app, bin, args)
}
