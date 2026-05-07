use serde::Deserialize;

use crate::errors::AppResult;
use crate::models::SvnStatusEntry;

use super::runner::run_svn;

#[derive(Debug, Deserialize)]
struct StatusRoot {
    #[serde(rename = "target", default)]
    targets: Vec<Target>,
}

#[derive(Debug, Deserialize)]
struct Target {
    #[serde(rename = "entry", default)]
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    #[serde(rename = "@path")]
    path: String,
    #[serde(rename = "wc-status")]
    wc_status: Option<WcStatus>,
}

#[derive(Debug, Deserialize)]
struct WcStatus {
    #[serde(rename = "@item")]
    item: String,
    #[serde(rename = "@props")]
    props: Option<String>,
    #[serde(rename = "@copied", default)]
    copied: Option<String>,
    #[serde(rename = "@revision")]
    revision: Option<u64>,
    commit: Option<Commit>,
}

#[derive(Debug, Deserialize)]
struct Commit {
    #[serde(rename = "@revision")]
    revision: u64,
    author: Option<String>,
    date: Option<String>,
}

/// 获取工作副本状态。show_unversioned=true 时显示未跟踪文件
pub fn svn_status(
    svn_bin: &str,
    target: &str,
    show_unversioned: bool,
) -> AppResult<Vec<SvnStatusEntry>> {
    let mut args = vec!["status", "--xml", "--non-interactive"];
    if !show_unversioned {
        args.push("--quiet");
    }
    args.push(target);

    let out = run_svn(svn_bin, &args)?;
    let parsed: StatusRoot = quick_xml::de::from_str(&out.stdout)?;

    let mut result = Vec::new();
    for target in parsed.targets {
        for entry in target.entries {
            let Some(wc) = entry.wc_status else { continue };
            // 默认情况下跳过 normal 项，避免大量噪声
            if wc.item == "normal" && wc.props.as_deref().unwrap_or("none") == "none" {
                continue;
            }
            let (commit_revision, commit_author, commit_date) = match wc.commit {
                Some(c) => (Some(c.revision), c.author, c.date),
                None => (None, None, None),
            };
            result.push(SvnStatusEntry {
                path: entry.path,
                item: wc.item,
                props: wc.props,
                copied: wc.copied.as_deref() == Some("true"),
                revision: wc.revision,
                commit_revision,
                commit_author,
                commit_date,
            });
        }
    }
    Ok(result)
}
