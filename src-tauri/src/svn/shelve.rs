use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::svn::runner::run_svn;

// 手动搁置统一放在 shelves/manual 下，与合并会话的 UUID 目录隔离
const MANUAL_DIR: &str = "manual";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfEntry {
    pub id: String,
    pub name: String,
    pub wc_path: String,
    pub created_at: String,
    pub file_count: usize,
    pub status_text: String,
}

pub fn wc_status_text(svn_bin: &str, path: &str) -> String {
    status_text_multi(svn_bin, &[path])
}

fn status_text_multi(svn_bin: &str, targets: &[&str]) -> String {
    let mut args = vec!["status"];
    args.extend(targets);
    run_svn(svn_bin, &args)
        .map(|o| o.stdout.trim().to_string())
        .unwrap_or_default()
}

// 是否为版本化修改行（首列不是 ? / I / X）
fn is_versioned_change_line(line: &str) -> bool {
    let trimmed = line.trim_end();
    if trimmed.trim().is_empty() {
        return false;
    }
    !matches!(trimmed.chars().next(), Some('?') | Some('I') | Some('X'))
}

pub fn has_versioned_changes(status: &str) -> bool {
    status.lines().any(is_versioned_change_line)
}

fn manual_root(shelves_dir: &Path) -> PathBuf {
    shelves_dir.join(MANUAL_DIR)
}

fn shelf_dir(shelves_dir: &Path, id: &str) -> AppResult<PathBuf> {
    // 编号由后端 UUID 生成，这里限制字符避免目录穿越
    if id.is_empty() || !id.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-') {
        return Err(AppError::Other("无效的搁置编号".into()));
    }
    Ok(manual_root(shelves_dir).join(id))
}

fn meta_path(dir: &Path) -> PathBuf {
    dir.join("meta.json")
}

/// 把工作副本的版本化修改保存为补丁并还原工作区（类似 git stash）。
/// paths 非空时只搁置这些文件，否则整个工作副本。
pub fn create_shelf(
    svn_bin: &str,
    wc_path: &str,
    name: &str,
    paths: &[String],
    shelves_dir: &Path,
) -> AppResult<ShelfEntry> {
    for p in paths {
        if !Path::new(p).starts_with(wc_path) {
            return Err(AppError::Other(format!("文件不在该工作副本内：{p}")));
        }
    }
    let targets: Vec<&str> = if paths.is_empty() {
        vec![wc_path]
    } else {
        paths.iter().map(String::as_str).collect()
    };

    let status = status_text_multi(svn_bin, &targets);
    if !has_versioned_changes(&status) {
        return Err(AppError::Other("没有可搁置的版本化修改".into()));
    }
    let mut diff_args = vec!["diff"];
    diff_args.extend(&targets);
    let diff = run_svn(svn_bin, &diff_args)?;
    // 二进制修改无法由 svn diff 完整保存，禁止继续以避免本地内容丢失
    if diff.stdout.contains("Cannot display: file marked as a binary type") {
        return Err(AppError::Other(
            "包含二进制修改，无法安全搁置，请先单独处理二进制文件".into(),
        ));
    }
    if diff.stdout.trim().is_empty() {
        return Err(AppError::Other("未能生成有效补丁，已保留工作副本修改".into()));
    }

    let id = Uuid::new_v4().to_string();
    let dir = shelf_dir(shelves_dir, &id)?;
    fs::create_dir_all(&dir)?;

    let name = name.trim();
    let entry = ShelfEntry {
        id,
        name: if name.is_empty() {
            format!("{} 的搁置", Local::now().format("%m-%d %H:%M"))
        } else {
            name.to_string()
        },
        wc_path: wc_path.to_string(),
        created_at: Local::now().to_rfc3339(),
        file_count: status.lines().filter(|l| is_versioned_change_line(l)).count(),
        status_text: status.clone(),
    };
    fs::write(dir.join("changes.patch"), &diff.stdout)?;
    fs::write(dir.join("status.txt"), format!("{status}\n"))?;
    fs::write(meta_path(&dir), serde_json::to_string_pretty(&entry)?)?;

    // 补丁落盘成功后才清理工作区；清理失败则整个搁置目录作废，保持修改原样
    let mut revert_args = vec!["revert", "-R"];
    revert_args.extend(&targets);
    if let Err(e) = run_svn(svn_bin, &revert_args) {
        let _ = fs::remove_dir_all(&dir);
        return Err(e);
    }
    remove_reverted_added_files(&status, wc_path);
    Ok(entry)
}

// svn revert 不会删除新增（A）状态的实体文件；留在磁盘上会让恢复补丁时被判定
// 「已应用」而丢失 add 调度，按 stash 语义在搁置后清掉这些文件
fn remove_reverted_added_files(status: &str, wc_path: &str) {
    let mut dirs = Vec::new();
    for line in status.lines() {
        if !line.starts_with('A') {
            continue;
        }
        let Some(p) = line.get(8..).map(str::trim) else {
            continue;
        };
        let path = Path::new(p);
        // 只处理 SVN 返回的工作副本内绝对路径，范围不明的一律跳过
        if !path.is_absolute() || !path.starts_with(wc_path) {
            continue;
        }
        if path.is_file() {
            let _ = fs::remove_file(path);
        } else if path.is_dir() {
            dirs.push(path.to_path_buf());
        }
    }
    // 先删深层目录；仅删空目录，避免误删目录内未版本化文件
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    for dir in dirs {
        let _ = fs::remove_dir(dir);
    }
}

/// 列出手动搁置，可按工作副本路径过滤，按创建时间倒序。
pub fn list_shelves(shelves_dir: &Path, wc_path: Option<&str>) -> AppResult<Vec<ShelfEntry>> {
    let mut entries = Vec::new();
    let Ok(read) = fs::read_dir(manual_root(shelves_dir)) else {
        return Ok(entries);
    };
    for item in read.flatten() {
        let Ok(text) = fs::read_to_string(meta_path(&item.path())) else {
            continue;
        };
        let Ok(entry) = serde_json::from_str::<ShelfEntry>(&text) else {
            continue;
        };
        if let Some(filter) = wc_path {
            if entry.wc_path != filter {
                continue;
            }
        }
        entries.push(entry);
    }
    entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(entries)
}

/// 应用搁置补丁到其来源工作副本。应用成功后保留记录，由用户确认无误再删除。
pub fn apply_shelf(svn_bin: &str, shelves_dir: &Path, id: &str) -> AppResult<String> {
    let dir = shelf_dir(shelves_dir, id)?;
    let meta = fs::read_to_string(meta_path(&dir))?;
    let entry: ShelfEntry = serde_json::from_str(&meta)?;
    let patch = dir.join("changes.patch");
    if !patch.exists() {
        return Err(AppError::Other("搁置补丁文件不存在".into()));
    }
    let patch_text = patch.to_string_lossy().to_string();
    let out = run_svn(svn_bin, &["patch", &patch_text, &entry.wc_path])?;
    Ok(out.stdout)
}

pub fn delete_shelf(shelves_dir: &Path, id: &str) -> AppResult<()> {
    let dir = shelf_dir(shelves_dir, id)?;
    if !meta_path(&dir).exists() {
        return Err(AppError::Other("找不到对应的搁置记录".into()));
    }
    fs::remove_dir_all(&dir)?;
    Ok(())
}
