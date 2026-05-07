use serde::Deserialize;

use crate::errors::AppResult;
use crate::models::RemoteListEntry;

use super::runner::run_svn;

#[derive(Debug, Deserialize)]
struct ListsRoot {
    #[serde(rename = "list", default)]
    lists: Vec<ListXml>,
}

#[derive(Debug, Deserialize)]
struct ListXml {
    #[serde(rename = "entry", default)]
    entries: Vec<EntryXml>,
}

#[derive(Debug, Deserialize)]
struct EntryXml {
    #[serde(rename = "@kind")]
    kind: String,
    name: String,
    size: Option<u64>,
    commit: Option<CommitXml>,
}

#[derive(Debug, Deserialize)]
struct CommitXml {
    #[serde(rename = "@revision")]
    revision: Option<u64>,
    author: Option<String>,
    date: Option<String>,
}

fn join_url(base: &str, name: &str) -> String {
    if name.is_empty() {
        return base.to_string();
    }
    format!("{}/{}", base.trim_end_matches('/'), name)
}

pub fn svn_list_remote(
    svn_bin: &str,
    url: &str,
    username: Option<&str>,
) -> AppResult<Vec<RemoteListEntry>> {
    let mut args: Vec<String> = vec!["list".into(), "--xml".into(), "--non-interactive".into()];
    if let Some(u) = username {
        if !u.trim().is_empty() {
            args.push("--username".into());
            args.push(u.to_string());
        }
    }
    args.push(url.to_string());
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = run_svn(svn_bin, &arg_refs)?;
    let parsed: ListsRoot = quick_xml::de::from_str(&out.stdout)?;

    let mut result = Vec::new();
    for list in parsed.lists {
        for entry in list.entries {
            let (revision, author, date) = match entry.commit {
                Some(commit) => (commit.revision, commit.author, commit.date),
                None => (None, None, None),
            };
            let path = entry.name.trim_end_matches('/').to_string();
            result.push(RemoteListEntry {
                name: path
                    .rsplit('/')
                    .next()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&path)
                    .to_string(),
                url: join_url(url, &path),
                path,
                kind: entry.kind,
                size: entry.size,
                revision,
                author,
                date,
            });
        }
    }
    result.sort_by(|a, b| match (a.kind.as_str(), b.kind.as_str()) {
        ("dir", "file") => std::cmp::Ordering::Less,
        ("file", "dir") => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(result)
}

pub fn svn_cat_remote(svn_bin: &str, url: &str, username: Option<&str>) -> AppResult<String> {
    let mut args: Vec<String> = vec!["cat".into(), "--non-interactive".into()];
    if let Some(u) = username {
        if !u.trim().is_empty() {
            args.push("--username".into());
            args.push(u.to_string());
        }
    }
    args.push(url.to_string());
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = run_svn(svn_bin, &arg_refs)?;
    Ok(out.stdout)
}
