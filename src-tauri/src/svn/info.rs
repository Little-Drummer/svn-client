use serde::Deserialize;

use crate::errors::{AppError, AppResult};
use crate::models::SvnInfo;

use super::runner::{run_svn, run_svn_with_timeout, REMOTE_SVN_TIMEOUT};

// 这些结构体仅用于反序列化 svn info --xml 输出，对外用 SvnInfo
#[derive(Debug, Deserialize)]
struct InfoRoot {
    #[serde(rename = "entry", default)]
    entries: Vec<InfoEntry>,
}

#[derive(Debug, Deserialize)]
struct InfoEntry {
    #[serde(rename = "@kind")]
    kind: String,
    #[serde(rename = "@path")]
    path: String,
    #[serde(rename = "@revision")]
    revision: u64,
    url: String,
    #[serde(rename = "relative-url")]
    relative_url: Option<String>,
    repository: Repository,
    commit: Option<Commit>,
}

#[derive(Debug, Deserialize)]
struct Repository {
    root: String,
    uuid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Commit {
    #[serde(rename = "@revision")]
    revision: u64,
    author: Option<String>,
    date: Option<String>,
}

pub fn svn_info(svn_bin: &str, target: &str) -> AppResult<SvnInfo> {
    let args = ["info", "--xml", "--non-interactive", target];
    // 远端 URL 查询走短超时，避免未连 VPN 时长时间等待
    let out = if target.contains("://") {
        run_svn_with_timeout(svn_bin, &args, REMOTE_SVN_TIMEOUT)?
    } else {
        run_svn(svn_bin, &args)?
    };
    parse_svn_info_xml(&out.stdout)
}

pub fn parse_svn_info_xml(xml: &str) -> AppResult<SvnInfo> {
    let parsed: InfoRoot = quick_xml::de::from_str(xml)?;
    let entry = parsed
        .entries
        .into_iter()
        .next()
        .ok_or_else(|| AppError::XmlParse("svn info 返回为空".into()))?;

    let (last_changed_revision, last_changed_author, last_changed_date) = match entry.commit {
        Some(c) => (Some(c.revision), c.author, c.date),
        None => (None, None, None),
    };

    Ok(SvnInfo {
        path: entry.path,
        url: entry.url,
        repository_root: entry.repository.root,
        repository_uuid: entry.repository.uuid,
        revision: entry.revision,
        kind: entry.kind,
        relative_url: entry.relative_url,
        last_changed_revision,
        last_changed_author,
        last_changed_date,
    })
}
