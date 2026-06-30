use serde::Deserialize;

use crate::errors::AppResult;
use crate::models::{SvnLogEntry, SvnLogPath};

use super::runner::run_svn;

#[derive(Debug, Deserialize)]
struct LogRoot {
    #[serde(rename = "logentry", default)]
    entries: Vec<LogEntryXml>,
}

#[derive(Debug, Deserialize)]
struct LogEntryXml {
    #[serde(rename = "@revision")]
    revision: u64,
    author: Option<String>,
    date: Option<String>,
    msg: Option<String>,
    paths: Option<PathsXml>,
}

#[derive(Debug, Deserialize)]
struct PathsXml {
    #[serde(rename = "path", default)]
    items: Vec<PathXml>,
}

#[derive(Debug, Deserialize)]
struct PathXml {
    #[serde(rename = "@action")]
    action: String,
    #[serde(rename = "@kind")]
    kind: Option<String>,
    #[serde(rename = "@copyfrom-path")]
    copyfrom_path: Option<String>,
    #[serde(rename = "@copyfrom-rev")]
    copyfrom_rev: Option<u64>,
    #[serde(rename = "$text")]
    path: Option<String>,
}

pub struct LogOptions<'a> {
    pub target: &'a str,
    pub limit: u32,
    pub revision_range: Option<&'a str>, // 如 "HEAD:1"
    pub search: Option<&'a str>,
    pub author: Option<&'a str>,
    pub date_from: Option<&'a str>,
    pub date_to: Option<&'a str>,
    pub with_paths: bool,
}

pub fn svn_log(svn_bin: &str, opts: &LogOptions) -> AppResult<Vec<SvnLogEntry>> {
    let mut args: Vec<String> = vec![
        "log".into(),
        "--xml".into(),
        "--non-interactive".into(),
        "--limit".into(),
        opts.limit.to_string(),
    ];

    if opts.with_paths {
        args.push("--verbose".into());
    }
    let date_range = match (opts.date_from, opts.date_to) {
        (Some(from), Some(to)) if !from.trim().is_empty() && !to.trim().is_empty() => {
            Some(format!("{{{}}}:{{{}}}", to.trim(), from.trim()))
        }
        (Some(from), _) if !from.trim().is_empty() => Some(format!("HEAD:{{{}}}", from.trim())),
        (_, Some(to)) if !to.trim().is_empty() => Some(format!("{{{}}}:1", to.trim())),
        _ => None,
    };

    // 工作副本未 update 时，svn log 默认只看到本地基准版本；显式 HEAD:1 才能拉取远端最新历史。
    let range = opts
        .revision_range
        .filter(|range| !range.trim().is_empty())
        .or(date_range.as_deref())
        .unwrap_or("HEAD:1");
    args.push("-r".into());
    args.push(range.into());
    if let Some(s) = opts.search {
        if !s.is_empty() {
            args.push("--search".into());
            args.push(s.into());
        }
    }
    args.push(opts.target.into());

    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = run_svn(svn_bin, &arg_refs)?;
    let parsed: LogRoot = quick_xml::de::from_str(&out.stdout)?;

    let mut result: Vec<SvnLogEntry> = parsed
        .entries
        .into_iter()
        .map(|e| SvnLogEntry {
            revision: e.revision,
            author: e.author,
            date: e.date,
            message: e.msg,
            paths: e
                .paths
                .map(|p| {
                    p.items
                        .into_iter()
                        .map(|x| SvnLogPath {
                            path: x.path.unwrap_or_default(),
                            action: x.action,
                            kind: x.kind,
                            copyfrom_path: x.copyfrom_path,
                            copyfrom_rev: x.copyfrom_rev,
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect();
    if let Some(author) = opts.author {
        let needle = author.trim().to_lowercase();
        if !needle.is_empty() {
            result.retain(|entry| {
                entry
                    .author
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&needle)
            });
        }
    }
    Ok(result)
}
