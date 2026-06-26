use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::AppResult;
use crate::models::{Project, ProjectModule};
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
    let mut selected: Vec<&MergeRevision> =
        entries.iter().filter(|e| set.contains(&e.revision)).collect();
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

// 把目标工作副本的版本化修改搁置到 shelves_dir，返回搁置目录（无修改返回 None）
fn shelve_target(
    svn_bin: &str,
    route: &MergeRoute,
    shelves_dir: &Path,
    on_line: &mut impl FnMut(StreamLine),
) -> Result<Option<PathBuf>, String> {
    let target = &route.target_path;
    let status = wc_status_text(svn_bin, target);
    if status.is_empty() || !has_versioned_changes(&status) {
        info(on_line, "目标工作副本没有需要搁置的修改。");
        return Ok(None);
    }

    let branch = route.personal_branch.clone().unwrap_or_else(|| {
        Path::new(target)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("target")
            .to_string()
    });
    let timestamp = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
    let shelf_dir = shelves_dir.join(format!("{branch}-{timestamp}"));
    fs::create_dir_all(&shelf_dir).map_err(|e| format!("创建搁置目录失败：{e}"))?;

    fs::write(shelf_dir.join("status.txt"), format!("{status}\n"))
        .map_err(|e| format!("写入状态文件失败：{e}"))?;

    let diff = run_svn(svn_bin, &["diff", target]).map_err(|e| format!("生成搁置补丁失败：{e}"))?;
    fs::write(shelf_dir.join("changes.patch"), format!("{}\n", diff.stdout))
        .map_err(|e| format!("写入补丁失败：{e}"))?;

    info(on_line, format!("已生成搁置补丁：{}", shelf_dir.display()));
    if !run_step(svn_bin, &["revert", "-R", target], on_line) {
        return Err("清理工作副本（svn revert）失败".into());
    }
    Ok(Some(shelf_dir))
}

fn restore_shelf(
    svn_bin: &str,
    target: &str,
    shelf_dir: &Path,
    on_line: &mut impl FnMut(StreamLine),
) {
    let patch = shelf_dir.join("changes.patch");
    if !patch.exists() {
        on_line(StreamLine::Stderr(format!(
            "未找到搁置补丁：{}",
            patch.display()
        )));
        return;
    }
    let patch_str = patch.to_string_lossy().to_string();
    if run_step(svn_bin, &["patch", &patch_str, target], on_line) {
        info(on_line, "已恢复搁置的本地修改。");
    } else {
        on_line(StreamLine::Stderr("恢复搁置补丁失败，请手动处理。".into()));
    }
}

/// 完整合并流程：（可选搁置）→ update → merge → 有变更才 commit →（恢复搁置）。
/// 每一步的输出通过 on_line 流式回传。返回整体是否成功。
pub fn run_merge_flow<F: FnMut(StreamLine)>(
    svn_bin: &str,
    route: &MergeRoute,
    revisions: &[u64],
    message: &str,
    shelves_dir: &Path,
    mut on_line: F,
) -> bool {
    let target = &route.target_path;

    // 1. develop/rest -> 个人分支 时，先搁置个人分支的本地修改
    let mut shelf: Option<PathBuf> = None;
    if route.sync_branch {
        info(&mut on_line, "检查是否需要搁置目标本地修改...");
        match shelve_target(svn_bin, route, shelves_dir, &mut on_line) {
            Ok(p) => shelf = p,
            Err(e) => {
                on_line(StreamLine::Stderr(e));
                return false;
            }
        }
    }

    // 2. update，避免 mixed-revision 工作副本导致合并失败
    info(&mut on_line, "开始执行 svn update...");
    if !run_step(svn_bin, &["update", "--non-interactive", target], &mut on_line) {
        on_line(StreamLine::Stderr("svn update 失败，已中止合并。".into()));
        return false;
    }
    let before = wc_status_text(svn_bin, target);

    // 3. merge
    info(&mut on_line, "开始执行 svn merge...");
    let url = source_url(svn_bin, &route.source_path);
    let arg = merge_revision_arg(revisions);
    if !run_step(
        svn_bin,
        &["merge", "--non-interactive", "-c", &arg, &url, target],
        &mut on_line,
    ) {
        on_line(StreamLine::Stderr("合并失败或存在冲突，请处理后再继续。".into()));
        return false;
    }

    let after = wc_status_text(svn_bin, target);
    if after == before {
        info(
            &mut on_line,
            "合并完成，但没有文件进入待提交状态，跳过提交。",
        );
        if let Some(dir) = &shelf {
            restore_shelf(svn_bin, target, dir, &mut on_line);
        }
        return true;
    }

    // 4. commit
    info(&mut on_line, "开始执行 svn commit...");
    if !run_step(
        svn_bin,
        &["commit", "--non-interactive", target, "-m", message],
        &mut on_line,
    ) {
        on_line(StreamLine::Stderr(
            "提交失败，请根据输出处理。搁置代码（如有）尚未恢复。".into(),
        ));
        return false;
    }

    // 5. 恢复搁置
    if let Some(dir) = &shelf {
        info(&mut on_line, "开始恢复搁置的本地修改...");
        restore_shelf(svn_bin, target, dir, &mut on_line);
    }

    info(&mut on_line, "合并流程完成。");
    true
}
