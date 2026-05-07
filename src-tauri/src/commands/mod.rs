use chrono::Utc;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::{
    RemoteListEntry, RepositoryEntry, SvnInfo, SvnLogEntry, SvnStatusEntry, WorkingCopyEntry,
    WorkingCopyFileEntry,
};
use crate::process::spawn_svn_task;
use crate::storage::ConfigState;
use crate::svn::diff::{svn_cat_base, svn_diff_file, svn_diff_revision};
use crate::svn::info::svn_info;
use crate::svn::list::{
    svn_cat_remote as run_svn_cat_remote, svn_list_remote as run_svn_list_remote,
};
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

// ---------- 远端仓库管理 ----------

#[tauri::command]
pub fn list_repositories(state: State<ConfigState>) -> AppResult<Vec<RepositoryEntry>> {
    let cfg = state
        .config
        .lock()
        .map_err(|_| AppError::Other("config 锁被污染".into()))?;
    Ok(cfg.repositories.clone())
}

#[tauri::command]
pub fn save_repository(
    state: State<ConfigState>,
    id: Option<String>,
    name: String,
    url: String,
    username: Option<String>,
) -> AppResult<RepositoryEntry> {
    let name = name.trim().to_string();
    let url = url.trim().to_string();
    if name.is_empty() || url.is_empty() {
        return Err(AppError::Other("仓库名称和 URL 不能为空".into()));
    }

    let entry = RepositoryEntry {
        id: id.unwrap_or_else(|| Uuid::new_v4().to_string()),
        name,
        url,
        username: username.filter(|s| !s.trim().is_empty()),
        last_accessed_at: None,
    };

    {
        let mut cfg = state
            .config
            .lock()
            .map_err(|_| AppError::Other("config 锁被污染".into()))?;
        if let Some(existing) = cfg.repositories.iter_mut().find(|repo| repo.id == entry.id) {
            existing.name = entry.name.clone();
            existing.url = entry.url.clone();
            existing.username = entry.username.clone();
            let result = existing.clone();
            drop(cfg);
            state.save()?;
            return Ok(result);
        }
        cfg.repositories.push(entry.clone());
    }
    state.save()?;
    Ok(entry)
}

#[tauri::command]
pub fn remove_repository(state: State<ConfigState>, id: String) -> AppResult<()> {
    {
        let mut cfg = state
            .config
            .lock()
            .map_err(|_| AppError::Other("config 锁被污染".into()))?;
        cfg.repositories.retain(|repo| repo.id != id);
    }
    state.save()
}

#[tauri::command]
pub fn test_repository_connection(
    state: State<ConfigState>,
    id: Option<String>,
    url: String,
    username: Option<String>,
) -> AppResult<SvnInfo> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err(AppError::Other("仓库 URL 不能为空".into()));
    }
    let bin = state.svn_bin();
    let mut args: Vec<String> = vec!["info".into(), "--xml".into(), "--non-interactive".into()];
    if let Some(u) = username {
        if !u.trim().is_empty() {
            args.push("--username".into());
            args.push(u);
        }
    }
    args.push(url.clone());
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = run_svn(&bin, &arg_refs)?;
    let info = crate::svn::info::parse_svn_info_xml(&out.stdout)?;

    if let Some(repo_id) = id {
        let mut cfg = state
            .config
            .lock()
            .map_err(|_| AppError::Other("config 锁被污染".into()))?;
        if let Some(repo) = cfg.repositories.iter_mut().find(|repo| repo.id == repo_id) {
            repo.last_accessed_at = Some(Utc::now().to_rfc3339());
        }
        drop(cfg);
        state.save()?;
    }

    Ok(info)
}

#[tauri::command]
pub fn svn_list_remote(
    state: State<ConfigState>,
    url: String,
    username: Option<String>,
) -> AppResult<Vec<RemoteListEntry>> {
    if url.trim().is_empty() {
        return Err(AppError::Other("远端 URL 不能为空".into()));
    }
    let bin = state.svn_bin();
    run_svn_list_remote(&bin, url.trim(), username.as_deref())
}

#[tauri::command]
pub fn svn_cat_remote(
    state: State<ConfigState>,
    url: String,
    username: Option<String>,
) -> AppResult<String> {
    if url.trim().is_empty() {
        return Err(AppError::Other("远端文件 URL 不能为空".into()));
    }
    let bin = state.svn_bin();
    run_svn_cat_remote(&bin, url.trim(), username.as_deref())
}

#[tauri::command]
pub fn list_working_copies(state: State<ConfigState>) -> AppResult<Vec<WorkingCopyEntry>> {
    let cfg = state
        .config
        .lock()
        .map_err(|_| AppError::Other("config 锁被污染".into()))?;
    Ok(cfg.working_copies.clone())
}

#[tauri::command]
pub fn add_working_copy(state: State<ConfigState>, path: String) -> AppResult<WorkingCopyEntry> {
    if path.trim().is_empty() {
        return Err(AppError::InvalidPath(path));
    }
    let bin = state.svn_bin();
    // 校验是 svn working copy
    let info = svn_info(&bin, &path).map_err(|e| match e {
        AppError::SvnCommand { stderr, .. }
            if stderr.to_lowercase().contains("not a working copy") =>
        {
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
        if let Some(existing) = cfg
            .working_copies
            .iter_mut()
            .find(|wc| wc.path == entry.path)
        {
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
pub fn refresh_working_copy(state: State<ConfigState>, id: String) -> AppResult<WorkingCopyEntry> {
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

#[tauri::command]
pub fn list_working_copy_files(root: String) -> AppResult<Vec<WorkingCopyFileEntry>> {
    use std::path::{Path, PathBuf};

    fn build_entry(path: PathBuf, root: &Path) -> AppResult<Option<WorkingCopyFileEntry>> {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        if name == ".svn" {
            return Ok(None);
        }
        let meta = std::fs::metadata(&path).map_err(AppError::Io)?;
        let kind = if meta.is_dir() { "dir" } else { "file" }.to_string();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let mut children = Vec::new();
        if meta.is_dir() {
            let mut child_paths = Vec::new();
            for entry in std::fs::read_dir(&path).map_err(AppError::Io)? {
                child_paths.push(entry.map_err(AppError::Io)?.path());
            }
            child_paths.sort_by(|a, b| {
                let a_is_dir = a.is_dir();
                let b_is_dir = b.is_dir();
                match (a_is_dir, b_is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase()
                        .cmp(
                            &b.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_lowercase(),
                        ),
                }
            });
            for child in child_paths {
                if let Some(entry) = build_entry(child, root)? {
                    children.push(entry);
                }
            }
        }
        Ok(Some(WorkingCopyFileEntry {
            name,
            path: path.to_string_lossy().to_string(),
            relative_path: rel,
            kind,
            children,
        }))
    }

    let root_path = PathBuf::from(root.trim());
    if root_path.as_os_str().is_empty() || !root_path.is_dir() {
        return Err(AppError::InvalidPath(root));
    }
    let mut result = Vec::new();
    for entry in std::fs::read_dir(&root_path).map_err(AppError::Io)? {
        if let Some(file) = build_entry(entry.map_err(AppError::Io)?.path(), &root_path)? {
            result.push(file);
        }
    }
    Ok(result)
}

#[tauri::command]
pub fn create_working_copy_folder(parent_path: String, name: String) -> AppResult<String> {
    use std::path::PathBuf;

    let name = name.trim();
    if name.is_empty() || name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err(AppError::Other("文件夹名称不合法".into()));
    }
    let parent = PathBuf::from(parent_path.trim());
    if !parent.is_dir() {
        return Err(AppError::InvalidPath(parent_path));
    }
    let target = parent.join(name);
    std::fs::create_dir(&target).map_err(AppError::Io)?;
    Ok(target.to_string_lossy().to_string())
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
    author: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    with_paths: Option<bool>,
) -> AppResult<Vec<SvnLogEntry>> {
    let bin = state.svn_bin();
    let opts = LogOptions {
        target: &path,
        limit: limit.unwrap_or(50).min(500),
        revision_range: revision_range.as_deref(),
        search: search.as_deref(),
        author: author.as_deref(),
        date_from: date_from.as_deref(),
        date_to: date_to.as_deref(),
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

#[tauri::command]
pub fn svn_add(state: State<ConfigState>, paths: Vec<String>) -> AppResult<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let bin = state.svn_bin();
    let mut args: Vec<&str> = vec!["add", "--parents", "--non-interactive"];
    for p in &paths {
        args.push(p);
    }
    run_svn(&bin, &args)?;
    Ok(())
}

#[tauri::command]
pub fn svn_delete(state: State<ConfigState>, paths: Vec<String>) -> AppResult<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let bin = state.svn_bin();
    let mut args: Vec<&str> = vec!["delete", "--force", "--non-interactive"];
    for p in &paths {
        args.push(p);
    }
    run_svn(&bin, &args)?;
    Ok(())
}

#[tauri::command]
pub fn svn_ignore(state: State<ConfigState>, paths: Vec<String>) -> AppResult<()> {
    use std::collections::BTreeMap;
    use std::path::Path;

    if paths.is_empty() {
        return Ok(());
    }
    let bin = state.svn_bin();
    let mut by_parent: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in paths {
        let path = Path::new(&p);
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let parent = path
            .parent()
            .and_then(|s| s.to_str())
            .unwrap_or(".")
            .to_string();
        by_parent.entry(parent).or_default().push(name.to_string());
    }

    for (parent, names) in by_parent {
        let current = run_svn(&bin, &["propget", "svn:ignore", &parent])
            .map(|out| out.stdout)
            .unwrap_or_default();
        let mut lines: Vec<String> = current
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect();
        for name in names {
            if !lines.iter().any(|line| line == &name) {
                lines.push(name);
            }
        }
        let value = lines.join("\n");
        run_svn(&bin, &["propset", "svn:ignore", &value, &parent])?;
    }
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
    for path in &paths {
        let statuses = svn_status(&bin, path, true).unwrap_or_default();
        if statuses.iter().any(|entry| entry.item == "conflicted") {
            return Err(AppError::Other("存在冲突文件，解决冲突后才能提交".into()));
        }
    }
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
