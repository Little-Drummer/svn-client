use chrono::{DateTime, Utc};
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::{
    CapturePresetFile, ConfigPreset, PresetApplyPlan, Project, RemoteListEntry, RepositoryEntry,
    SvnInfo, SvnLogEntry, SvnStatusEntry, WorkingCopyEntry, WorkingCopyFileEntry,
};
use crate::process::{spawn_merge_task, spawn_status_stream, spawn_svn_task, ProcessRegistry};
use crate::storage::ConfigState;
use crate::svn::diff::{svn_cat_base, svn_cat_revision, svn_diff_file, svn_diff_revision};
use crate::svn::info::svn_info;
use crate::svn::list::{
    svn_cat_remote as run_svn_cat_remote, svn_list_remote as run_svn_list_remote,
};
use crate::svn::log::{svn_log, LogOptions};
use crate::svn::merge::{
    build_preview, build_routes, fetch_revisions, MergePreview, MergeRevision, MergeRoute,
};
use crate::svn::package::{
    commit_version, fetch_package_revisions, package_zip, run_package_build, PackageBuildResult,
    PackageOptions, PackageRevision, PackageZipResult,
};
use crate::svn::project::{group_working_copies, scan_project_dir};
use crate::svn::runner::{check_svn_version, run_svn};
use crate::svn::status::{svn_status, svn_status_verbose_all};

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
    let mut entries = cfg.working_copies.clone();
    // 实时标记路径可用性：外置卷未挂载或目录被删时前端据此降级展示，而不是逐个命令报错
    for wc in &mut entries {
        wc.available = std::path::Path::new(&wc.path).is_dir();
    }
    Ok(entries)
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
        relative_url: info.relative_url.clone(),
        display_name: None,
        available: true,
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
            existing.relative_url = entry.relative_url.clone();
            // 保留用户之前设置的 display_name，不要覆盖
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
    entry.relative_url = info.relative_url.clone();
    // display_name 由用户手动维护，刷新时不覆盖
    let result = entry.clone();
    drop(cfg);
    state.save()?;
    Ok(result)
}

/// 仅更新工作副本的显示名称（用户自定义别名），不重新执行 svn info
#[tauri::command]
pub fn set_working_copy_display_name(
    state: State<ConfigState>,
    id: String,
    display_name: Option<String>,
) -> AppResult<WorkingCopyEntry> {
    let mut cfg = state
        .config
        .lock()
        .map_err(|_| AppError::Other("config 锁被污染".into()))?;

    let entry = cfg
        .working_copies
        .iter_mut()
        .find(|wc| wc.id == id)
        .ok_or_else(|| AppError::Other(format!("找不到工作副本 {}", id)))?;

    entry.display_name = if display_name
        .as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true)
    {
        None
    } else {
        display_name
    };

    let result = entry.clone();
    drop(cfg);
    state.save()?;
    Ok(result)
}

/// 项目分组视图：把当前工作副本聚合成 项目 → 环境(分支) → 模块 结构。
#[tauri::command]
pub fn list_projects(state: State<ConfigState>) -> AppResult<Vec<Project>> {
    let cfg = state
        .config
        .lock()
        .map_err(|_| AppError::Other("config 锁被污染".into()))?;
    Ok(group_working_copies(&cfg.working_copies))
}

/// 扫描一个项目根目录，自动识别其下所有分支/模块工作副本并批量加入列表。
/// 已存在的路径只更新元信息，返回本次新增或更新的工作副本。
#[tauri::command]
pub fn scan_and_add_project(
    state: State<ConfigState>,
    path: String,
) -> AppResult<Vec<WorkingCopyEntry>> {
    if path.trim().is_empty() {
        return Err(AppError::InvalidPath(path));
    }
    let bin = state.svn_bin();
    let root = std::path::PathBuf::from(path.trim());
    let candidates = scan_project_dir(&root)?;

    // 先在锁外把每个候选目录的 svn info 取齐，避免持锁期间执行子进程
    let mut infos = Vec::new();
    for wc_path in candidates {
        let p = wc_path.to_string_lossy().to_string();
        // 个别子目录不是有效工作副本时跳过，不影响其余识别结果
        if let Ok(info) = svn_info(&bin, &p) {
            infos.push(info);
        }
    }

    let now = Utc::now().to_rfc3339();
    let mut touched = Vec::new();
    {
        let mut cfg = state
            .config
            .lock()
            .map_err(|_| AppError::Other("config 锁被污染".into()))?;
        for info in infos {
            if let Some(existing) = cfg
                .working_copies
                .iter_mut()
                .find(|wc| wc.path == info.path)
            {
                existing.url = Some(info.url);
                existing.repository_root = Some(info.repository_root);
                existing.revision = Some(info.revision);
                existing.last_seen_at = Some(now.clone());
                existing.relative_url = info.relative_url;
                // 保留用户设置的 display_name
                touched.push(existing.clone());
            } else {
                let entry = WorkingCopyEntry {
                    id: Uuid::new_v4().to_string(),
                    path: info.path,
                    url: Some(info.url),
                    repository_root: Some(info.repository_root),
                    revision: Some(info.revision),
                    last_seen_at: Some(now.clone()),
                    relative_url: info.relative_url,
                    display_name: None,
                    available: true,
                };
                cfg.working_copies.push(entry.clone());
                touched.push(entry);
            }
        }
    }
    state.save()?;
    Ok(touched)
}

// ---------- 多级合并 ----------

/// 列出某个项目所有可用的合并方向。
#[tauri::command]
pub fn merge_list_routes(
    state: State<ConfigState>,
    project_name: String,
) -> AppResult<Vec<MergeRoute>> {
    let cfg = state
        .config
        .lock()
        .map_err(|_| AppError::Other("config 锁被污染".into()))?;
    let projects = group_working_copies(&cfg.working_copies);
    let project = projects
        .iter()
        .find(|p| p.name == project_name)
        .ok_or_else(|| AppError::Other(format!("找不到项目 {project_name}")))?;
    Ok(build_routes(project))
}

/// 拉取某条合并方向下目标分支尚未包含的、可合入的版本（已过滤双向同步提交）。
#[tauri::command]
pub async fn merge_fetch_revisions(
    state: State<'_, ConfigState>,
    route: MergeRoute,
) -> AppResult<Vec<MergeRevision>> {
    let bin = state.svn_bin();
    tauri::async_runtime::spawn_blocking(move || fetch_revisions(&bin, &route))
        .await
        .map_err(|e| AppError::Other(format!("拉取合并版本任务失败: {e}")))?
}

/// 根据选中版本生成等价命令行 + 公司格式合并日志预览。
#[tauri::command]
pub async fn merge_preview(
    state: State<'_, ConfigState>,
    route: MergeRoute,
    entries: Vec<MergeRevision>,
    revisions: Vec<u64>,
) -> AppResult<MergePreview> {
    let bin = state.svn_bin();
    tauri::async_runtime::spawn_blocking(move || build_preview(&bin, &route, &entries, &revisions))
        .await
        .map_err(|e| AppError::Other(format!("生成合并预览任务失败: {e}")))
}

/// 执行合并：（可选搁置）→ update → merge → 有变更才 commit →（恢复搁置），流式推送进度。
#[tauri::command]
pub fn merge_execute(
    app: AppHandle,
    state: State<ConfigState>,
    route: MergeRoute,
    revisions: Vec<u64>,
    message: String,
) -> AppResult<String> {
    if revisions.is_empty() {
        return Err(AppError::Other("没有选择任何版本".into()));
    }
    let bin = state.svn_bin();
    let shelves_dir = state
        .config_path
        .parent()
        .map(|p| p.join("shelves"))
        .ok_or_else(|| AppError::Other("无法定位配置目录".into()))?;
    spawn_merge_task(app, bin, route, revisions, message, shelves_dir)
}

// ---------- 本地开发配置预设 ----------

/// 列出全部配置预设。预设是全局统一的，不再按项目过滤。
#[tauri::command]
pub fn list_config_presets(state: State<ConfigState>) -> AppResult<Vec<ConfigPreset>> {
    let cfg = state
        .config
        .lock()
        .map_err(|_| AppError::Other("config 锁被污染".into()))?;
    Ok(cfg.config_presets.clone())
}

/// 把选中文件的当前内容捕获为一个配置预设。
/// 每个文件可指定行范围（只捕获并替换那几行），不指定则整文件。
#[tauri::command]
pub fn capture_config_preset(
    state: State<ConfigState>,
    name: String,
    wc_root: String,
    files: Vec<CapturePresetFile>,
) -> AppResult<ConfigPreset> {
    if name.trim().is_empty() {
        return Err(AppError::Other("预设名称不能为空".into()));
    }
    if files.is_empty() {
        return Err(AppError::Other("至少选择一个文件".into()));
    }
    let root = std::path::PathBuf::from(wc_root.trim());
    let mut preset_files = Vec::new();
    for spec in &files {
        preset_files.push(crate::svn::preset::capture_file(&root, spec)?);
    }
    let preset = ConfigPreset {
        id: Uuid::new_v4().to_string(),
        project_name: None,
        name: name.trim().to_string(),
        files: preset_files,
    };
    {
        let mut cfg = state
            .config
            .lock()
            .map_err(|_| AppError::Other("config 锁被污染".into()))?;
        cfg.config_presets.push(preset.clone());
    }
    state.save()?;
    Ok(preset)
}

fn find_preset(state: &State<ConfigState>, id: &str) -> AppResult<ConfigPreset> {
    let cfg = state
        .config
        .lock()
        .map_err(|_| AppError::Other("config 锁被污染".into()))?;
    cfg.config_presets
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| AppError::Other(format!("找不到预设 {id}")))
}

/// 预览应用预设：返回每个文件将发生的变更（含被替换的行），不写盘。
#[tauri::command]
pub fn preview_config_preset(
    state: State<ConfigState>,
    id: String,
    wc_root: String,
) -> AppResult<Vec<PresetApplyPlan>> {
    let preset = find_preset(&state, &id)?;
    let root = std::path::PathBuf::from(wc_root.trim());
    crate::svn::preset::apply_preset(&root, &preset.files, true)
}

/// 应用预设到目标工作副本：整文件覆盖或按片段行替换，返回每个文件的结果。
#[tauri::command]
pub fn apply_config_preset(
    state: State<ConfigState>,
    id: String,
    wc_root: String,
) -> AppResult<Vec<PresetApplyPlan>> {
    let preset = find_preset(&state, &id)?;
    let root = std::path::PathBuf::from(wc_root.trim());
    crate::svn::preset::apply_preset(&root, &preset.files, false)
}

/// 删除一个配置预设。
#[tauri::command]
pub fn delete_config_preset(state: State<ConfigState>, id: String) -> AppResult<()> {
    {
        let mut cfg = state
            .config
            .lock()
            .map_err(|_| AppError::Other("config 锁被污染".into()))?;
        cfg.config_presets.retain(|p| p.id != id);
    }
    state.save()
}

// ---------- 增量打包 ----------

/// 拉取 rest 模块最近若干条提交，供选择打包版本。
#[tauri::command]
pub async fn package_fetch_revisions(
    state: State<'_, ConfigState>,
    rest_path: String,
    limit: Option<u32>,
) -> AppResult<Vec<PackageRevision>> {
    let bin = state.svn_bin();
    let lim = limit.unwrap_or(20);
    tauri::async_runtime::spawn_blocking(move || fetch_package_revisions(&bin, &rest_path, lim))
        .await
        .map_err(|e| AppError::Other(format!("拉取打包版本任务失败: {e}")))?
}

/// 执行增量包构建（不含 zip）。
#[tauri::command]
pub async fn package_build(
    state: State<'_, ConfigState>,
    rest_path: String,
    options: PackageOptions,
    revisions: Vec<u64>,
) -> AppResult<PackageBuildResult> {
    let bin = state.svn_bin();
    tauri::async_runtime::spawn_blocking(move || {
        run_package_build(&bin, &rest_path, &options, &revisions)
    })
    .await
    .map_err(|e| AppError::Other(format!("构建增量包任务失败: {e}")))?
    .map_err(AppError::Other)
}

/// 把提供包目录打成 ZIP（构建完成、放入前端包后调用）。
#[tauri::command]
pub async fn package_make_zip(
    base_dir: String,
    requirement_name: String,
) -> AppResult<PackageZipResult> {
    tauri::async_runtime::spawn_blocking(move || package_zip(&base_dir, &requirement_name))
        .await
        .map_err(|e| AppError::Other(format!("打包 ZIP 任务失败: {e}")))?
        .map_err(AppError::Other)
}

/// 写入分支 rest/version 并提交。
#[tauri::command]
pub async fn package_commit_version(
    state: State<'_, ConfigState>,
    rest_path: String,
    version: String,
) -> AppResult<String> {
    let bin = state.svn_bin();
    tauri::async_runtime::spawn_blocking(move || commit_version(&bin, &rest_path, &version))
        .await
        .map_err(|e| AppError::Other(format!("提交 version 任务失败: {e}")))?
}

#[tauri::command]
pub async fn list_working_copy_files(
    state: State<'_, ConfigState>,
    root: String,
) -> AppResult<Vec<WorkingCopyFileEntry>> {
    let bin = state.svn_bin();
    // 全量递归遍历目录树是阻塞 IO，必须移出主线程，否则切换工作副本时会冻结 UI
    tauri::async_runtime::spawn_blocking(move || list_working_copy_files_blocking(root, bin))
        .await
        .map_err(|e| AppError::Other(format!("文件树遍历任务失败: {e}")))?
}

fn list_working_copy_files_blocking(
    root: String,
    svn_bin: String,
) -> AppResult<Vec<WorkingCopyFileEntry>> {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    fn normalize_status_path(root: &Path, raw_path: &str) -> String {
        let p = PathBuf::from(raw_path);
        if p.is_absolute() {
            p.to_string_lossy().to_string()
        } else {
            root.join(p).to_string_lossy().to_string()
        }
    }

    fn build_entry(
        path: PathBuf,
        root: &Path,
        svn_meta: &HashMap<String, SvnStatusEntry>,
    ) -> AppResult<Option<WorkingCopyFileEntry>> {
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
        let modified_at = meta.modified().ok().map(|time| {
            let dt: DateTime<Utc> = time.into();
            dt.to_rfc3339()
        });
        let size = if meta.is_file() {
            Some(meta.len())
        } else {
            None
        };
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
                if let Some(entry) = build_entry(child, root, svn_meta)? {
                    children.push(entry);
                }
            }
        }
        let svn = svn_meta.get(&path.to_string_lossy().to_string());
        Ok(Some(WorkingCopyFileEntry {
            name,
            path: path.to_string_lossy().to_string(),
            relative_path: rel,
            kind,
            size,
            modified_at,
            svn_item: svn.map(|s| s.item.clone()),
            props: svn.and_then(|s| s.props.clone()),
            copied: svn.map(|s| s.copied).unwrap_or(false),
            revision: svn.and_then(|s| s.revision),
            commit_revision: svn.and_then(|s| s.commit_revision),
            commit_author: svn.and_then(|s| s.commit_author.clone()),
            commit_date: svn.and_then(|s| s.commit_date.clone()),
            children,
        }))
    }

    let root_path = PathBuf::from(root.trim());
    if root_path.as_os_str().is_empty() || !root_path.is_dir() {
        return Err(AppError::InvalidPath(root));
    }
    // verbose status 只用于补齐列表列；失败时仍展示本地文件树，避免 SVN 元数据阻塞浏览。
    let svn_meta: HashMap<String, SvnStatusEntry> = svn_status_verbose_all(&svn_bin, root.trim())
        .unwrap_or_default()
        .into_iter()
        .map(|entry| (normalize_status_path(&root_path, &entry.path), entry))
        .collect();
    let mut result = Vec::new();
    for entry in std::fs::read_dir(&root_path).map_err(AppError::Io)? {
        if let Some(file) = build_entry(entry.map_err(AppError::Io)?.path(), &root_path, &svn_meta)?
        {
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
pub async fn svn_get_info(state: State<'_, ConfigState>, path: String) -> AppResult<SvnInfo> {
    let bin = state.svn_bin();
    tauri::async_runtime::spawn_blocking(move || svn_info(&bin, &path))
        .await
        .map_err(|e| AppError::Other(format!("svn info 任务失败: {e}")))?
}

#[tauri::command]
pub async fn svn_get_status(
    state: State<'_, ConfigState>,
    path: String,
    show_unversioned: Option<bool>,
) -> AppResult<Vec<SvnStatusEntry>> {
    let bin = state.svn_bin();
    let show = show_unversioned.unwrap_or(true);
    tauri::async_runtime::spawn_blocking(move || svn_status(&bin, &path, show))
        .await
        .map_err(|e| AppError::Other(format!("svn status 任务失败: {e}")))?
}

// 流式刷新状态，立即返回 request_id，结果通过 svn-status-stream 事件推送
#[tauri::command]
pub fn svn_get_status_stream(
    app: AppHandle,
    state: State<ConfigState>,
    path: String,
    show_unversioned: Option<bool>,
) -> AppResult<String> {
    let bin = state.svn_bin();
    Ok(spawn_status_stream(
        app,
        bin,
        path,
        show_unversioned.unwrap_or(true),
    ))
}

#[tauri::command]
pub async fn svn_get_log(
    state: State<'_, ConfigState>,
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
    tauri::async_runtime::spawn_blocking(move || {
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
    })
    .await
    .map_err(|e| AppError::Other(format!("svn log 任务失败: {e}")))?
}

#[tauri::command]
pub async fn svn_get_diff(state: State<'_, ConfigState>, path: String) -> AppResult<String> {
    let bin = state.svn_bin();
    tauri::async_runtime::spawn_blocking(move || svn_diff_file(&bin, &path))
        .await
        .map_err(|e| AppError::Other(format!("svn diff 任务失败: {e}")))?
}

#[tauri::command]
pub async fn svn_get_diff_revision(
    state: State<'_, ConfigState>,
    path: String,
    revision: u64,
) -> AppResult<String> {
    let bin = state.svn_bin();
    tauri::async_runtime::spawn_blocking(move || svn_diff_revision(&bin, &path, revision))
        .await
        .map_err(|e| AppError::Other(format!("svn diff 任务失败: {e}")))?
}

#[tauri::command]
pub async fn svn_get_base_content(
    state: State<'_, ConfigState>,
    path: String,
) -> AppResult<String> {
    let bin = state.svn_bin();
    tauri::async_runtime::spawn_blocking(move || svn_cat_base(&bin, &path))
        .await
        .map_err(|e| AppError::Other(format!("svn cat 任务失败: {e}")))?
}

// 取目标（本地路径或远端 URL）在指定 revision 的完整内容，供 log 视图左右对比
#[tauri::command]
pub async fn svn_get_cat_revision(
    state: State<'_, ConfigState>,
    target: String,
    revision: u64,
) -> AppResult<String> {
    let bin = state.svn_bin();
    tauri::async_runtime::spawn_blocking(move || svn_cat_revision(&bin, &target, revision))
        .await
        .map_err(|e| AppError::Other(format!("svn cat 任务失败: {e}")))?
}

// 在系统文件管理器中定位文件（macOS Finder / Windows 资源管理器 / Linux 文件管理器）
#[tauri::command]
pub fn reveal_in_file_manager(path: String) -> AppResult<()> {
    use std::process::Command;
    let p = path.trim();
    if p.is_empty() {
        return Err(AppError::InvalidPath(path));
    }
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = Command::new("open");
        c.args(["-R", p]);
        c
    };
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("explorer");
        // explorer 的 /select 参数语法对斜杠敏感，交由系统按原样定位
        c.arg(format!("/select,{}", p));
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        // Linux 没有统一的"定位并高亮"，退而打开所在目录
        let parent = std::path::Path::new(p)
            .parent()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_else(|| p.to_string());
        let mut c = Command::new("xdg-open");
        c.arg(parent);
        c
    };
    cmd.spawn().map_err(AppError::Io)?;
    Ok(())
}

// 在系统终端中打开指定目录（macOS Terminal / Windows cmd / Linux 常见终端）
#[tauri::command]
pub fn open_in_terminal(path: String) -> AppResult<()> {
    use std::process::Command;
    let p = path.trim();
    if p.is_empty() {
        return Err(AppError::InvalidPath(path));
    }
    // 工作副本路径若指向文件则退回其所在目录，确保终端落在有效工作目录
    let dir = {
        let pb = std::path::Path::new(p);
        if pb.is_dir() {
            p.to_string()
        } else {
            pb.parent()
                .map(|d| d.to_string_lossy().to_string())
                .unwrap_or_else(|| p.to_string())
        }
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = Command::new("open");
        c.args(["-a", "Terminal", &dir]);
        c
    };
    #[cfg(target_os = "windows")]
    let mut cmd = {
        // start 是 cmd 内建命令，需经 cmd /c 调用；空标题参数占位避免路径被当成窗口标题
        let mut c = Command::new("cmd");
        c.args(["/c", "start", "cmd", "/k", "cd", "/d", &dir]);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        // Linux 终端无统一入口，优先 x-terminal-emulator，失败时由调用方感知
        let mut c = Command::new("x-terminal-emulator");
        c.current_dir(&dir);
        c
    };
    cmd.spawn().map_err(AppError::Io)?;
    Ok(())
}

// 读取磁盘文件文本，用于 diff 视图的 split 模式（避免引入完整 fs plugin）
#[tauri::command]
pub async fn read_file_text(path: String) -> AppResult<String> {
    tauri::async_runtime::spawn_blocking(move || -> AppResult<String> {
        let bytes = std::fs::read(&path).map_err(AppError::Io)?;
        Ok(crate::svn::runner::decode_output(&bytes))
    })
    .await
    .map_err(|e| AppError::Other(format!("读取文件任务失败: {e}")))?
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
    registry: State<ProcessRegistry>,
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
    spawn_svn_task(app, bin, args, "提交".into(), registry.inner().clone())
}

#[tauri::command]
pub fn svn_start_update(
    app: AppHandle,
    state: State<ConfigState>,
    registry: State<ProcessRegistry>,
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
    spawn_svn_task(app, bin, args, "更新".into(), registry.inner().clone())
}

#[tauri::command]
pub fn svn_start_checkout(
    app: AppHandle,
    state: State<ConfigState>,
    registry: State<ProcessRegistry>,
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
    spawn_svn_task(app, bin, args, "检出".into(), registry.inner().clone())
}

/// 终止一个运行中的长任务（kill 其 svn 子进程）。
/// 返回任务当时是否还在运行（false 表示已结束或不存在，前端可据此忽略）。
#[tauri::command]
pub fn svn_cancel_task(registry: State<ProcessRegistry>, task_id: String) -> bool {
    registry.cancel(&task_id)
}

/// 清理工作副本残留的锁。kill 更新进程后工作副本会留在 locked 状态（E155004），
/// 此命令阻塞到 cleanup 完成，前端随后即可安全重试原任务。
#[tauri::command]
pub async fn svn_cleanup(state: State<'_, ConfigState>, path: String) -> AppResult<()> {
    let bin = state.svn_bin();
    tauri::async_runtime::spawn_blocking(move || {
        run_svn(&bin, &["cleanup", "--non-interactive", &path]).map(|_| ())
    })
    .await
    .map_err(|e| AppError::Other(format!("cleanup 执行线程异常: {e}")))?
}
