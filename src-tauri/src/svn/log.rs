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
    if let Some(range) = opts.revision_range {
        args.push("-r".into());
        args.push(range.into());
    }
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

    let result = parsed
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
    Ok(result)
}
