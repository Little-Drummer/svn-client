use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
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
pub fn run_svn_streaming<F>(svn_bin: &str, args: &[&str], on_line: F) -> AppResult<StreamResult>
where
    F: FnMut(StreamLine),
{
    // 不需要取消能力的调用走空的 on_spawn
    run_svn_streaming_cancellable(svn_bin, args, |_| {}, on_line)
}

/// 与 run_svn_streaming 相同，但在子进程启动后立即把句柄交给 on_spawn，
/// 调用方据此把进程登记到注册表，以便外部 kill（终止任务）。
/// 关键点：流式读取阶段（rx 循环）不持有 child 锁，因此外部 kill 能随时拿到锁；
/// kill 后管道 EOF、循环退出，再 wait 时进程已结束，不会与 kill 抢锁死锁。
pub fn run_svn_streaming_cancellable<S, F>(
    svn_bin: &str,
    args: &[&str],
    on_spawn: S,
    mut on_line: F,
) -> AppResult<StreamResult>
where
    S: FnOnce(Arc<Mutex<Child>>),
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

    // 管道已取出，可安全地把 child 交给注册表用于外部 kill
    let child = Arc::new(Mutex::new(child));
    on_spawn(child.clone());

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

    // 此时管道已 EOF（进程正常结束或被 kill），wait 不会长时间阻塞
    let status = child.lock().unwrap().wait()?;
    Ok(StreamResult {
        success: status.success(),
        exit_code: status.code(),
    })
}
