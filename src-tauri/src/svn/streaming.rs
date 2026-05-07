use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use encoding_rs::{GBK, UTF_8};

use crate::errors::{AppError, AppResult};

pub enum StreamLine {
    Stdout(String),
    Stderr(String),
}

pub struct StreamResult {
    pub success: bool,
    pub exit_code: Option<i32>,
}

fn decode_line(bytes: &[u8]) -> String {
    let (cow, _, had_errors) = UTF_8.decode(bytes);
    if !had_errors {
        return cow.into_owned();
    }
    let (cow, _, _) = GBK.decode(bytes);
    cow.into_owned()
}

/// 启动 svn 子进程，按行读取 stdout/stderr，每读到一行调用 on_line。
/// 调用是阻塞的，完成后返回最终状态。
pub fn run_svn_streaming<F>(svn_bin: &str, args: &[&str], mut on_line: F) -> AppResult<StreamResult>
where
    F: FnMut(StreamLine),
{
    let mut cmd = Command::new(svn_bin);
    cmd.args(args);
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

    let (tx, rx) = mpsc::channel::<StreamLine>();
    let tx_err = tx.clone();

    let h_out = thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    let line = decode_line(&buf).trim_end_matches(['\r', '\n']).to_string();
                    let _ = tx.send(StreamLine::Stdout(line));
                }
                Err(_) => break,
            }
        }
    });

    let h_err = thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    let line = decode_line(&buf).trim_end_matches(['\r', '\n']).to_string();
                    let _ = tx_err.send(StreamLine::Stderr(line));
                }
                Err(_) => break,
            }
        }
    });

    while let Ok(line) = rx.recv() {
        on_line(line);
    }

    let _ = h_out.join();
    let _ = h_err.join();

    let status = child.wait()?;
    Ok(StreamResult {
        success: status.success(),
        exit_code: status.code(),
    })
}
