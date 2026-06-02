use std::io::BufReader;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use quick_xml::events::Event;
use quick_xml::writer::Writer;
use quick_xml::Reader;
use serde::Deserialize;

use crate::errors::{AppError, AppResult};
use crate::models::SvnStatusEntry;

use super::runner::{decode_output, run_svn};

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
            if let Some(status) = entry_to_status(entry) {
                result.push(status);
            }
        }
    }
    Ok(result)
}

/// 把解析出的 entry 转为对外结构。默认跳过 normal 且无属性变更的项，避免大量噪声。
fn entry_to_status(entry: Entry) -> Option<SvnStatusEntry> {
    let wc = entry.wc_status?;
    if wc.item == "normal" && wc.props.as_deref().unwrap_or("none") == "none" {
        return None;
    }
    let (commit_revision, commit_author, commit_date) = match wc.commit {
        Some(c) => (Some(c.revision), c.author, c.date),
        None => (None, None, None),
    };
    Some(SvnStatusEntry {
        path: entry.path,
        item: wc.item,
        props: wc.props,
        copied: wc.copied.as_deref() == Some("true"),
        revision: wc.revision,
        commit_revision,
        commit_author,
        commit_date,
    })
}

/// 流式获取工作副本状态：spawn svn 子进程，用 quick-xml 增量解析 stdout，
/// 每解析出一个 <entry> 立即回调 on_entry，避免大工作副本等全量。
/// 调用阻塞，完成后返回；进程非成功退出时返回带 stderr 的错误。
pub fn svn_status_streaming<F>(
    svn_bin: &str,
    target: &str,
    show_unversioned: bool,
    mut on_entry: F,
) -> AppResult<()>
where
    F: FnMut(SvnStatusEntry),
{
    let mut args = vec!["status", "--xml", "--non-interactive"];
    if !show_unversioned {
        args.push("--quiet");
    }
    args.push(target);

    let mut cmd = Command::new(svn_bin);
    cmd.args(&args);
    cmd.env("LC_ALL", "en_US.UTF-8");
    cmd.env("LANG", "en_US.UTF-8");
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::null());

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::SvnNotFound(format!("找不到可执行文件 {}: {}", svn_bin, e))
        } else {
            AppError::Io(e)
        }
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Other("无法获取子进程 stdout".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Other("无法获取子进程 stderr".into()))?;

    // stderr 单开线程收集，结束后用于错误信息，避免管道写满阻塞子进程
    let (err_tx, err_rx) = mpsc::channel::<Vec<u8>>();
    let h_err = thread::spawn(move || {
        use std::io::Read;
        let mut reader = BufReader::new(stderr);
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf);
        let _ = err_tx.send(buf);
    });

    let mut reader = Reader::from_reader(BufReader::new(stdout));
    let mut buf = Vec::new();
    // 累积当前 <entry>...</entry> 子树，遇 </entry> 整体反序列化
    let mut entry_writer: Option<Writer<Vec<u8>>> = None;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) if e.name().as_ref() == b"entry" => {
                let mut w = Writer::new(Vec::new());
                w.write_event(Event::Start(e.borrow()))?;
                entry_writer = Some(w);
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"entry" => {
                if let Some(mut w) = entry_writer.take() {
                    w.write_event(Event::End(e.borrow()))?;
                    let bytes = w.into_inner();
                    let xml = decode_output(&bytes);
                    let entry: Entry = quick_xml::de::from_str(&xml)?;
                    if let Some(status) = entry_to_status(entry) {
                        on_entry(status);
                    }
                }
            }
            Ok(ev) => {
                // entry 内部的子元素（wc-status/commit/author/date 等）原样写入缓冲
                if let Some(w) = entry_writer.as_mut() {
                    w.write_event(ev.borrow())?;
                }
            }
            Err(e) => {
                let _ = child.kill();
                return Err(AppError::XmlParse(e.to_string()));
            }
        }
    }

    let status = child.wait()?;
    let _ = h_err.join();
    if !status.success() {
        let stderr_bytes = err_rx.recv().unwrap_or_default();
        return Err(AppError::SvnCommand {
            message: "svn status 执行失败".into(),
            stderr: decode_output(&stderr_bytes),
            exit_code: status.code(),
        });
    }
    Ok(())
}
