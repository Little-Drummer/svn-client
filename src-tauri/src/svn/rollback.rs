use std::path::Path;

use serde::Serialize;

use crate::errors::{AppError, AppResult};
use crate::svn::info::svn_info;
use crate::svn::runner::run_svn;
use crate::svn::shelve::{create_shelf, has_versioned_changes, wc_status_text, ShelfEntry};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackResult {
    pub output: String,
    pub conflicted: bool,
    // 回滚前自动搁置的记录，未搁置时为空
    pub shelf: Option<ShelfEntry>,
}

/// 反向合并撤销一笔提交，改动保留在工作副本，由用户检查后另行提交。
/// 工作副本有版本化修改且未允许自动搁置时报错，由前端引导用户选择。
pub fn rollback_revision(
    svn_bin: &str,
    wc_path: &str,
    revision: u64,
    auto_shelve: bool,
    shelves_dir: &Path,
) -> AppResult<RollbackResult> {
    let status = wc_status_text(svn_bin, wc_path);
    let mut shelf = None;
    if has_versioned_changes(&status) {
        if !auto_shelve {
            return Err(AppError::Other(
                "工作副本存在未提交修改，请先搁置或提交后再回滚".into(),
            ));
        }
        shelf = Some(create_shelf(
            svn_bin,
            wc_path,
            &format!("回滚 r{revision} 前自动搁置"),
            &[],
            shelves_dir,
        )?);
    }

    let url = svn_info(svn_bin, wc_path)?.url;
    let rev_arg = format!("-{revision}");
    let out = run_svn(
        svn_bin,
        &["merge", "--non-interactive", "-c", &rev_arg, &url, wc_path],
    )?;

    let after = wc_status_text(svn_bin, wc_path);
    let conflicted = after
        .lines()
        .any(|line| line.chars().take(7).any(|ch| ch == 'C'));
    Ok(RollbackResult {
        output: out.stdout,
        conflicted,
        shelf,
    })
}
