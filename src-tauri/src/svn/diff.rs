use crate::errors::AppResult;

use super::runner::run_svn;

/// 获取单个文件相对于 BASE 的 diff（unified 格式）
pub fn svn_diff_file(svn_bin: &str, target: &str) -> AppResult<String> {
    let out = run_svn(
        svn_bin,
        &["diff", "--non-interactive", "--internal-diff", target],
    )?;
    Ok(out.stdout)
}

/// 获取指定 revision 引入的 diff（用于 log 视图查看某个版本的改动）
pub fn svn_diff_revision(svn_bin: &str, target: &str, revision: u64) -> AppResult<String> {
    let range = format!("{}:{}", revision.saturating_sub(1).max(0), revision);
    let out = run_svn(
        svn_bin,
        &[
            "diff",
            "--non-interactive",
            "--internal-diff",
            "-r",
            &range,
            target,
        ],
    )?;
    Ok(out.stdout)
}

/// 读取本地文件原始字节（diff 视图想做 side-by-side 时需要 BASE 文件）
pub fn svn_cat_base(svn_bin: &str, target: &str) -> AppResult<String> {
    let out = run_svn(svn_bin, &["cat", "--non-interactive", "-r", "BASE", target])?;
    Ok(out.stdout)
}

/// 读取目标在指定 revision 的完整内容（log 视图做左右对比时取 N-1 / N 两份全文）
pub fn svn_cat_revision(svn_bin: &str, target: &str, revision: u64) -> AppResult<String> {
    let rev = revision.to_string();
    let out = run_svn(svn_bin, &["cat", "--non-interactive", "-r", &rev, target])?;
    Ok(out.stdout)
}
