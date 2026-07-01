use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::errors::AppResult;
use crate::models::{MergeRouteConfig, Project, ProjectModule};
use crate::svn::info::svn_info;
use crate::svn::log::{svn_log, LogOptions};
use crate::svn::runner::run_svn;
use crate::svn::streaming::{run_svn_streaming, StreamLine};

// 合并涉及的模块（与脚本 MERGE_MODULES 一致）
const MERGE_MODULES: &[&str] = &["rest", "database", "updatesql"];
// develop 下不算个人分支的目录名
const NON_BRANCH_DIRS: &[&str] = &[
    "rest",
    "database",
    "updatesql",
    "front",
    "frontend",
    "node_modules",
    "target",
    ".idea",
    ".svn",
    "branches",
];

// 一条合并方向。source/target 都是本地工作副本路径。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeRoute {
    pub id: String,
    pub name: String,
    pub source_label: String,
    pub source_path: String,
    pub target_path: String,
    pub kind: String,
    // 个人分支名：路由涉及个人分支时携带，用于过滤双向同步提交
    #[serde(default)]
    pub personal_branch: Option<String>,
    // develop/rest -> 个人分支 方向
    #[serde(default)]
    pub sync_branch: bool,
}

// 候选合并版本（已过滤双向同步提交，按版本号倒序）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeRevision {
    pub revision: u64,
    pub author: Option<String>,
    pub date: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergePreview {
    pub command: String,
    pub message: String,
}

fn find_module<'a>(project: &'a Project, env: &str, module: &str) -> Option<&'a ProjectModule> {
    project
        .branches
        .iter()
        .find(|b| b.environment.eq_ignore_ascii_case(env))
        .and_then(|b| {
            b.modules
                .iter()
                .find(|m| m.module.eq_ignore_ascii_case(module))
        })
}

fn personal_branches(project: &Project) -> Vec<&ProjectModule> {
    let Some(develop) = project
        .branches
        .iter()
        .find(|b| b.environment.eq_ignore_ascii_case("develop"))
    else {
        return Vec::new();
    };
    develop
        .modules
        .iter()
        .filter(|m| {
            let lower = m.module.to_lowercase();
            !NON_BRANCH_DIRS.contains(&lower.as_str())
        })
        .collect()
}

/// 根据项目结构推断所有可用合并方向，顺序与脚本一致：
/// 个人分支双向 → develop→test → develop→produce → test→produce（按模块）。
pub fn build_routes(project: &Project) -> Vec<MergeRoute> {
    let mut routes = Vec::new();

    // 个人分支：需要 develop/rest 作为主干
    if let Some(develop_rest) = find_module(project, "develop", "rest") {
        for branch in personal_branches(project) {
            let name = &branch.module;
            routes.push(MergeRoute {
                id: format!("personal:{name}:to-develop"),
                name: format!("develop/{name} -> develop/rest"),
                source_label: format!("develop/{name}"),
                source_path: branch.path.clone(),
                target_path: develop_rest.path.clone(),
                kind: "个人分支 -> develop/rest".into(),
                personal_branch: Some(name.clone()),
                sync_branch: false,
            });
            routes.push(MergeRoute {
                id: format!("personal:{name}:from-develop"),
                name: format!("develop/rest -> develop/{name}"),
                source_label: "develop/rest".into(),
                source_path: develop_rest.path.clone(),
                target_path: branch.path.clone(),
                kind: "develop/rest -> 个人分支".into(),
                personal_branch: Some(name.clone()),
                sync_branch: true,
            });
        }
    }

    for module in MERGE_MODULES {
        let develop = find_module(project, "develop", module);
        let test = find_module(project, "test", module);
        let produce = find_module(project, "produce", module);
        let suffix = format!("/{module}");

        if let (Some(d), Some(t)) = (develop, test) {
            routes.push(MergeRoute {
                id: if *module == "rest" {
                    "develop:test".into()
                } else {
                    format!("develop:test:{module}")
                },
                name: format!("develop{suffix} -> test{suffix}"),
                source_label: format!("develop{suffix}"),
                source_path: d.path.clone(),
                target_path: t.path.clone(),
                kind: format!("develop{suffix} -> test{suffix}"),
                personal_branch: None,
                sync_branch: false,
            });
        }
        if let (Some(d), Some(p)) = (develop, produce) {
            routes.push(MergeRoute {
                id: if *module == "rest" {
                    "develop:produce".into()
                } else {
                    format!("develop:produce:{module}")
                },
                name: format!("develop{suffix} -> produce{suffix}"),
                source_label: format!("develop{suffix}"),
                source_path: d.path.clone(),
                target_path: p.path.clone(),
                kind: format!("develop{suffix} -> produce{suffix}"),
                personal_branch: None,
                sync_branch: false,
            });
        }
        if let (Some(t), Some(p)) = (test, produce) {
            routes.push(MergeRoute {
                id: if *module == "rest" {
                    "test:produce".into()
                } else {
                    format!("test:produce:{module}")
                },
                name: format!("test{suffix} -> produce{suffix}"),
                source_label: format!("test{suffix}"),
                source_path: t.path.clone(),
                target_path: p.path.clone(),
                kind: format!("test{suffix} -> produce{suffix}"),
                personal_branch: None,
                sync_branch: false,
            });
        }
    }

    routes
}

pub fn build_configured_routes(
    owner_project: &Project,
    projects: &[Project],
    configs: &[MergeRouteConfig],
) -> Vec<MergeRoute> {
    let mut routes = Vec::new();
    for config in configs {
        if !config.enabled || config.project_name != owner_project.name {
            continue;
        }
        // 兼容升级前只记录 project_name 的同项目配置
        let source_project_name = if config.source_project_name.is_empty() {
            &config.project_name
        } else {
            &config.source_project_name
        };
        let target_project_name = if config.target_project_name.is_empty() {
            &config.project_name
        } else {
            &config.target_project_name
        };
        let Some(source_project) = projects
            .iter()
            .find(|p| p.name == source_project_name.as_str())
        else {
            continue;
        };
        let Some(target_project) = projects
            .iter()
            .find(|p| p.name == target_project_name.as_str())
        else {
            continue;
        };
        let Some(source) = find_module(source_project, &config.source_env, &config.source_module)
        else {
            continue;
        };
        let Some(target) = find_module(target_project, &config.target_env, &config.target_module)
        else {
            continue;
        };
        if source.path == target.path {
            continue;
        }
        let source_label = format!(
            "{}/{}/{}",
            source_project_name, config.source_env, config.source_module
        );
        let target_label = format!(
            "{}/{}/{}",
            target_project_name, config.target_env, config.target_module
        );
        let name = if config.name.trim().is_empty() {
            format!("{source_label} -> {target_label}")
        } else {
            config.name.clone()
        };
        routes.push(MergeRoute {
            id: format!("custom:{}", config.id),
            name,
            source_label,
            source_path: source.path.clone(),
            target_path: target.path.clone(),
            kind: "自定义方向".into(),
            personal_branch: None,
            sync_branch: false,
        });
    }
    routes
}

fn source_url(svn_bin: &str, source_path: &str) -> String {
    svn_info(svn_bin, source_path)
        .map(|i| i.url)
        .unwrap_or_else(|_| source_path.to_string())
}

// 解码 URL 的 path 部分（百分号编码 + 反斜杠归一），用于和提交说明里的仓库路径比较
fn decode_url_path(url: &str) -> String {
    let path = match url.split_once("://") {
        Some((_scheme, rest)) => match rest.split_once('/') {
            Some((_host, p)) => p.to_string(),
            None => String::new(),
        },
        None => url.to_string(),
    };
    percent_decode(&path)
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// 从合并提交说明里抽取「从 PATH:」中的源路径
fn extract_merge_source_path(message: &str) -> Option<String> {
    let idx = message.find('从')?;
    let after = message[idx + '从'.len_utf8()..].trim_start();
    // 「从」后必须紧跟空白才算合并说明（对齐脚本的 \s+）
    if message[idx + '从'.len_utf8()..]
        .chars()
        .next()
        .map(|c| !c.is_whitespace())
        .unwrap_or(true)
    {
        return None;
    }
    let end = after.find(':')?;
    Some(
        after[..end]
            .trim()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_string(),
    )
}

// 过滤掉双向同步提交，避免把刚同步过去的提交又显示出来
fn filter_sync_entries(
    svn_bin: &str,
    entries: Vec<MergeRevision>,
    route: &MergeRoute,
) -> Vec<MergeRevision> {
    if route.sync_branch {
        // develop/rest -> 个人分支：剔除「从个人分支合并到 develop」的提交
        if let Some(branch) = &route.personal_branch {
            let needle = format!("/develop/branches/rest/{branch}");
            return entries
                .into_iter()
                .filter(|e| {
                    extract_merge_source_path(&e.message)
                        .map(|src| !src.ends_with(&needle))
                        .unwrap_or(true)
                })
                .collect();
        }
    }
    if route.personal_branch.is_some() && !route.sync_branch {
        // 个人分支 -> develop/rest：剔除「从 develop 合并到个人分支」的同步提交
        let develop_path = decode_url_path(&source_url(svn_bin, &route.target_path));
        if !develop_path.is_empty() {
            return entries
                .into_iter()
                .filter(|e| {
                    extract_merge_source_path(&e.message)
                        .map(|src| src != develop_path)
                        .unwrap_or(true)
                })
                .collect();
        }
    }
    entries
}

fn parse_revision_lines(text: &str) -> Vec<u64> {
    let mut revs = Vec::new();
    for line in text.lines() {
        let t = line.trim().trim_start_matches('r');
        if let Ok(n) = t.parse::<u64>() {
            revs.push(n);
        }
    }
    revs.sort_unstable();
    revs.dedup();
    revs
}

/// 拉取目标分支尚未包含的、可合入的版本及其提交说明，已过滤同步提交、按版本号倒序。
pub fn fetch_revisions(svn_bin: &str, route: &MergeRoute) -> AppResult<Vec<MergeRevision>> {
    let url = source_url(svn_bin, &route.source_path);
    let out = run_svn(
        svn_bin,
        &[
            "mergeinfo",
            "--show-revs",
            "eligible",
            &url,
            &route.target_path,
        ],
    )?;
    let eligible = parse_revision_lines(&out.stdout);
    if eligible.is_empty() {
        return Ok(Vec::new());
    }
    let set: HashSet<u64> = eligible.iter().copied().collect();
    let range = format!("{}:{}", eligible[0], eligible[eligible.len() - 1]);

    let entries = svn_log(
        svn_bin,
        &LogOptions {
            target: &url,
            limit: 1000,
            revision_range: Some(&range),
            search: None,
            author: None,
            date_from: None,
            date_to: None,
            with_paths: false,
        },
    )?;

    let mut revisions: Vec<MergeRevision> = entries
        .into_iter()
        .filter(|e| set.contains(&e.revision))
        .map(|e| MergeRevision {
            revision: e.revision,
            author: e.author,
            date: e.date,
            message: e
                .message
                .filter(|m| !m.trim().is_empty())
                .unwrap_or_else(|| "(无提交说明)".into()),
        })
        .collect();

    revisions = filter_sync_entries(svn_bin, revisions, route);
    revisions.sort_by(|a, b| b.revision.cmp(&a.revision));
    Ok(revisions)
}

// 压缩连续版本号：[136219,136681,136682,137158] -> "136219, 136681-136682, 137158"
fn compact_revisions(revisions: &[u64]) -> String {
    let mut nums: Vec<u64> = revisions.to_vec();
    nums.sort_unstable();
    nums.dedup();
    if nums.is_empty() {
        return String::new();
    }
    let mut ranges = Vec::new();
    let mut start = nums[0];
    let mut prev = nums[0];
    for &n in &nums[1..] {
        if n == prev + 1 {
            prev = n;
            continue;
        }
        ranges.push(if start != prev {
            format!("{start}-{prev}")
        } else {
            start.to_string()
        });
        start = n;
        prev = n;
    }
    ranges.push(if start != prev {
        format!("{start}-{prev}")
    } else {
        start.to_string()
    });
    ranges.join(", ")
}

fn merge_revision_arg(revisions: &[u64]) -> String {
    compact_revisions(revisions).replace(' ', "")
}

// 生成公司格式合并日志
fn format_merge_message(
    svn_bin: &str,
    route: &MergeRoute,
    entries: &[MergeRevision],
    revisions: &[u64],
) -> String {
    let set: HashSet<u64> = revisions.iter().copied().collect();
    let mut selected: Vec<&MergeRevision> = entries
        .iter()
        .filter(|e| set.contains(&e.revision))
        .collect();
    selected.sort_by_key(|e| e.revision);

    let revision_text = compact_revisions(revisions);
    let source_text = decode_url_path(&source_url(svn_bin, &route.source_path));
    let mut lines = vec![format!("合并了修改版本号{revision_text} 从 {source_text}:")];
    for entry in selected {
        let msg_lines: Vec<&str> = entry
            .message
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();
        if msg_lines.is_empty() {
            lines.push("(无提交说明)".into());
        } else {
            for l in msg_lines {
                lines.push(l.to_string());
            }
        }
        lines.push("........".into());
    }
    lines.join("\n")
}

/// 生成等价命令行 + 公司格式合并日志，供前端预览。
pub fn build_preview(
    svn_bin: &str,
    route: &MergeRoute,
    entries: &[MergeRevision],
    revisions: &[u64],
) -> MergePreview {
    let url = source_url(svn_bin, &route.source_path);
    let command = format!(
        "svn merge -c {} {} {}",
        merge_revision_arg(revisions),
        url,
        route.target_path
    );
    let message = format_merge_message(svn_bin, route, entries, revisions);
    MergePreview { command, message }
}

// ---------- 执行流程 ----------

fn wc_status_text(svn_bin: &str, path: &str) -> String {
    run_svn(svn_bin, &["status", path])
        .map(|o| o.stdout.trim().to_string())
        .unwrap_or_default()
}

// 是否存在版本化修改（首列不是 ? / I / X）
fn has_versioned_changes(status: &str) -> bool {
    status.lines().any(|line| {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            return false;
        }
        !matches!(trimmed.chars().next(), Some('?') | Some('I') | Some('X'))
    })
}

fn run_step<F: FnMut(StreamLine)>(svn_bin: &str, args: &[&str], on_line: &mut F) -> bool {
    match run_svn_streaming(svn_bin, args, |l| on_line(l)) {
        Ok(r) => r.success,
        Err(e) => {
            on_line(StreamLine::Stderr(e.to_string()));
            false
        }
    }
}

fn info(on_line: &mut impl FnMut(StreamLine), msg: impl Into<String>) {
    on_line(StreamLine::Stdout(msg.into()));
}

fn merge_shelf_dir(shelves_dir: &Path, session_id: &str) -> AppResult<PathBuf> {
    // 会话编号由后端 UUID 生成，这里继续限制字符，避免目录穿越
    if session_id.is_empty()
        || !session_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        return Err(crate::errors::AppError::Other("无效的合并会话编号".into()));
    }
    Ok(shelves_dir.join(session_id))
}

// 将目标工作副本的版本化修改保存为补丁，并清理工作副本供合并使用
fn shelve_target(
    svn_bin: &str,
    target: &str,
    shelf_dir: &Path,
    on_line: &mut impl FnMut(StreamLine),
) -> Result<bool, String> {
    fs::create_dir_all(shelf_dir).map_err(|e| format!("创建搁置目录失败：{e}"))?;
    fs::write(shelf_dir.join("target.txt"), target)
        .map_err(|e| format!("记录搁置目标失败：{e}"))?;
    let status = wc_status_text(svn_bin, target);
    if !has_versioned_changes(&status) {
        info(on_line, "目标工作副本没有需要搁置的版本化修改。");
        return Ok(false);
    }

    fs::write(shelf_dir.join("status.txt"), format!("{status}\n"))
        .map_err(|e| format!("写入搁置状态失败：{e}"))?;
    let diff = run_svn(svn_bin, &["diff", target])
        .map_err(|e| format!("生成搁置补丁失败：{e}"))?;
    // 二进制修改无法由 svn diff 完整保存，禁止继续清理以避免本地内容丢失
    if diff.stdout.contains("Cannot display: file marked as a binary type") {
        return Err("目标工作副本包含二进制修改，无法安全自动搁置，请先单独处理".into());
    }
    if diff.stdout.trim().is_empty() {
        return Err("未能生成有效搁置补丁，已保留目标工作副本原修改".into());
    }
    fs::write(shelf_dir.join("changes.patch"), diff.stdout)
        .map_err(|e| format!("写入搁置补丁失败：{e}"))?;

    info(on_line, format!("已搁置目标工作副本修改：{}", shelf_dir.display()));
    if !run_step(svn_bin, &["revert", "-R", target], on_line) {
        return Err("清理目标工作副本失败，搁置补丁已保留".into());
    }
    Ok(true)
}

/// 恢复指定合并会话的搁置补丁；没有搁置内容时返回 false。
pub fn restore_merge_shelf(
    svn_bin: &str,
    target: &str,
    shelves_dir: &Path,
    session_id: &str,
) -> AppResult<bool> {
    let shelf_dir = merge_shelf_dir(shelves_dir, session_id)?;
    let patch = shelf_dir.join("changes.patch");
    if !patch.exists() {
        return Ok(false);
    }
    let recorded_target = fs::read_to_string(shelf_dir.join("target.txt"))?;
    if recorded_target != target {
        return Err(crate::errors::AppError::Other(
            "搁置记录与目标工作副本不匹配".into(),
        ));
    }
    let patch_text = patch.to_string_lossy().to_string();
    run_svn(svn_bin, &["patch", &patch_text, target])?;
    // 保留已恢复补丁作为兜底，同时避免重复应用
    fs::rename(&patch, shelf_dir.join("changes.patch.restored"))?;
    Ok(true)
}

/// 撤销当前目标工作副本中的合并结果，然后恢复合并前搁置的修改。
pub fn rollback_merge(
    svn_bin: &str,
    target: &str,
    shelves_dir: &Path,
    session_id: &str,
) -> AppResult<bool> {
    let shelf_dir = merge_shelf_dir(shelves_dir, session_id)?;
    let recorded_target = fs::read_to_string(shelf_dir.join("target.txt"))?;
    if recorded_target != target {
        return Err(crate::errors::AppError::Other(
            "合并会话与目标工作副本不匹配，已拒绝撤销".into(),
        ));
    }
    let before_revert = wc_status_text(svn_bin, target);
    let original_status = fs::read_to_string(shelf_dir.join("status.txt")).unwrap_or_default();
    let original_added: HashSet<String> = added_status_paths(&original_status).into_iter().collect();
    let merge_added = added_status_paths(&before_revert);
    run_svn(svn_bin, &["revert", "-R", target])?;
    // svn revert 不会删除原本处于 added 状态的实体文件，需清理本次合并新增项
    for added in merge_added {
        if original_added.contains(&added) {
            continue;
        }
        let path = Path::new(&added);
        // 只清理 SVN 返回的目标工作副本内绝对路径，拒绝处理范围不明的相对路径
        if !path.is_absolute() || !path.starts_with(target) {
            continue;
        }
        if path.is_file() {
            fs::remove_file(path)?;
        } else if path.is_dir() {
            // 仅删除空目录，避免误删目录内已有的未版本化文件
            let _ = fs::remove_dir(path);
        }
    }
    restore_merge_shelf(svn_bin, target, shelves_dir, session_id)
}

fn added_status_paths(status: &str) -> Vec<String> {
    status
        .lines()
        .filter(|line| line.starts_with('A'))
        .filter_map(|line| line.get(8..).map(str::trim))
        .filter(|path| !path.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// 自动搁置目标已有修改，再执行 update → merge；提交和搁置恢复由前端分步触发。
pub fn run_merge_flow<F: FnMut(StreamLine)>(
    svn_bin: &str,
    route: &MergeRoute,
    revisions: &[u64],
    shelves_dir: &Path,
    session_id: &str,
    mut on_line: F,
) -> bool {
    let target = &route.target_path;

    let shelf_dir = match merge_shelf_dir(shelves_dir, session_id) {
        Ok(path) => path,
        Err(e) => {
            on_line(StreamLine::Stderr(e.to_string()));
            return false;
        }
    };
    // 先搁置目标已有修改，保证合并结果可以独立检查和提交
    if let Err(e) = shelve_target(svn_bin, target, &shelf_dir, &mut on_line) {
        on_line(StreamLine::Stderr(e));
        return false;
    }

    // 1. update，避免 mixed-revision 工作副本导致合并失败
    info(&mut on_line, "开始执行 svn update...");
    if !run_step(
        svn_bin,
        &["update", "--non-interactive", target],
        &mut on_line,
    ) {
        on_line(StreamLine::Stderr(
            "svn update 失败，已中止合并；可恢复搁置文件。".into(),
        ));
        return false;
    }
    // 2. merge
    info(&mut on_line, "开始执行 svn merge...");
    let url = source_url(svn_bin, &route.source_path);
    let arg = merge_revision_arg(revisions);
    if !run_step(
        svn_bin,
        &["merge", "--non-interactive", "-c", &arg, &url, target],
        &mut on_line,
    ) {
        on_line(StreamLine::Stderr(
            "合并失败或存在冲突，可撤销本次合并并恢复搁置文件。".into(),
        ));
        return false;
    }

    let status = wc_status_text(svn_bin, target);
    if status.lines().any(|line| line.chars().take(7).any(|ch| ch == 'C')) {
        on_line(StreamLine::Stderr(
            "检测到合并冲突，可撤销本次合并并恢复搁置文件。".into(),
        ));
        return false;
    }

    info(&mut on_line, "合并完成，变更已保留在目标工作副本，请检查后单独提交。");
    true
}
