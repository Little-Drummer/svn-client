use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::{AppError, AppResult};
use crate::svn::runner::run_svn;

// 只反序列化冲突相关字段，其余 svn info 字段忽略
#[derive(Debug, Deserialize)]
struct InfoRoot {
    #[serde(rename = "entry", default)]
    entries: Vec<InfoEntry>,
}

#[derive(Debug, Deserialize)]
struct InfoEntry {
    conflict: Option<ConflictNode>,
    #[serde(rename = "tree-conflict")]
    tree_conflict: Option<TreeConflictNode>,
}

#[derive(Debug, Deserialize)]
struct ConflictNode {
    // 冲突三方的落盘文件：base=共同基线，mine=本地版本，theirs=对方版本
    #[serde(rename = "prev-base-file")]
    prev_base_file: Option<String>,
    #[serde(rename = "prev-wc-file")]
    prev_wc_file: Option<String>,
    #[serde(rename = "cur-base-file")]
    cur_base_file: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TreeConflictNode {
    #[serde(rename = "@reason")]
    reason: Option<String>,
    #[serde(rename = "@action")]
    action: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictDetails {
    pub path: String,
    // 带冲突标记的当前工作文件内容
    pub working_content: String,
    pub mine_content: Option<String>,
    pub theirs_content: Option<String>,
    pub base_content: Option<String>,
}

// 冲突侧文件在 svn info 里可能是相对文件名，也可能是绝对路径
fn resolve_side_file(conflicted: &Path, name: &str) -> Option<PathBuf> {
    let p = Path::new(name);
    if p.is_absolute() {
        return p.exists().then(|| p.to_path_buf());
    }
    let joined = conflicted.parent()?.join(name);
    joined.exists().then_some(joined)
}

fn read_side(conflicted: &Path, name: &Option<String>) -> Option<String> {
    let name = name.as_deref()?;
    let path = resolve_side_file(conflicted, name)?;
    fs::read(&path)
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// 读取文本冲突的三方内容和当前工作文件内容。
pub fn conflict_details(svn_bin: &str, path: &str) -> AppResult<ConflictDetails> {
    let out = run_svn(svn_bin, &["info", "--xml", "--non-interactive", path])?;
    let parsed: InfoRoot = quick_xml::de::from_str(&out.stdout)?;
    let entry = parsed
        .entries
        .into_iter()
        .next()
        .ok_or_else(|| AppError::XmlParse("svn info 返回为空".into()))?;

    if let Some(tree) = entry.tree_conflict {
        return Err(AppError::Other(format!(
            "该文件是树冲突（{} / {}），暂不支持在界面中解决，请使用命令行处理",
            tree.action.unwrap_or_else(|| "未知操作".into()),
            tree.reason.unwrap_or_else(|| "未知原因".into()),
        )));
    }
    let conflict = entry
        .conflict
        .ok_or_else(|| AppError::Other("该文件当前没有文本冲突".into()))?;

    let file = Path::new(path);
    let working_content =
        fs::read(file).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())?;

    Ok(ConflictDetails {
        path: path.to_string(),
        working_content,
        mine_content: read_side(file, &conflict.prev_wc_file),
        theirs_content: read_side(file, &conflict.cur_base_file),
        base_content: read_side(file, &conflict.prev_base_file),
    })
}

const ACCEPT_CHOICES: &[&str] = &[
    "working",
    "mine-full",
    "theirs-full",
    "base",
    "mine-conflict",
    "theirs-conflict",
];

/// 标记冲突已解决。content 提供时先把编辑结果写回文件，只允许配合 working 方式。
pub fn resolve_conflict(
    svn_bin: &str,
    path: &str,
    accept: &str,
    content: Option<&str>,
) -> AppResult<()> {
    if !ACCEPT_CHOICES.contains(&accept) {
        return Err(AppError::Other(format!("不支持的解决方式：{accept}")));
    }
    if let Some(text) = content {
        if accept != "working" {
            return Err(AppError::Other(
                "自定义内容只能配合 working 方式使用".into(),
            ));
        }
        fs::write(path, text)?;
    }
    run_svn(
        svn_bin,
        &["resolve", "--accept", accept, "--non-interactive", path],
    )?;
    Ok(())
}
