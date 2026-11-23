use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use encoding_rs::{GBK, UTF_8};

use crate::errors::{AppError, AppResult};

/// 默认同步命令超时时间，避免网络异常时进程永久挂死
const DEFAULT_SVN_TIMEOUT: Duration = Duration::from_secs(120);

/// 解析 svn 输出可能的编码：先按 UTF-8 严格解码，失败再退回 GBK（Windows 中文环境常见）
pub fn decode_output(bytes: &[u8]) -> String {
    let (cow, _, had_errors) = UTF_8.decode(bytes);
    if !had_errors {
        return cow.into_owned();
    }
    let (cow, _, _) = GBK.decode(bytes);
    cow.into_owned()
}

fn build_command(svn_bin: &str, args: &[&str]) -> Command {
    let mut cmd = Command::new(svn_bin);
    cmd.args(args);
    // 强制 svn 输出英文，方便日志和错误提示稳定
    cmd.env("LC_ALL", "en_US.UTF-8");
    cmd.env("LANG", "en_US.UTF-8");
    cmd
}

pub struct SvnOutput {
    pub stdout: String,
    pub stderr: String,
}

/// 一次性执行命令，等待结束后返回输出（带默认超时保护）
pub fn run_svn(svn_bin: &str, args: &[&str]) -> AppResult<SvnOutput> {
    run_svn_with_timeout(svn_bin, args, DEFAULT_SVN_TIMEOUT)
}

/// 带超时控制执行 svn 命令；超时后强制终止子进程，防止网络抖动导致长期卡死
pub fn run_svn_with_timeout(
    svn_bin: &str,
    args: &[&str],
    timeout: Duration,
) -> AppResult<SvnOutput> {
    let mut cmd = build_command(svn_bin, args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::SvnNotFound(format!("找不到可执行文件 {}: {}", svn_bin, e))
        } else {
            AppError::Io(e)
        }
    })?;

    // 独立线程并发消费 stdout/stderr，避免输出较大时撑满管道缓冲区导致子进程阻塞死锁
    let stdout_handle = child.stdout.take().map(|mut pipe| {
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            buf
        })
    });
    let stderr_handle = child.stderr.take().map(|mut pipe| {
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            buf
        })
    });

    let start = Instant::now();
    let status = loop {
        match child.try_wait().map_err(AppError::Io)? {
            Some(status) => break status,
            None => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    if let Some(h) = stdout_handle {
                        let _ = h.join();
                    }
                    if let Some(h) = stderr_handle {
                        let _ = h.join();
                    }
                    return Err(AppError::SvnCommand {
                        message: format!(
                            "svn {} 执行超时（超过 {} 秒）",
                            args.first().copied().unwrap_or(""),
                            timeout.as_secs()
                        ),
                        stderr: "请检查网络连接或 SVN 服务器状态".to_string(),
                        exit_code: None,
                    });
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    };

    let stdout_bytes = stdout_handle
        .and_then(|h| h.join().ok())
        .unwrap_or_default();
    let stderr_bytes = stderr_handle
        .and_then(|h| h.join().ok())
        .unwrap_or_default();

    let stdout = decode_output(&stdout_bytes);
    let stderr = decode_output(&stderr_bytes);
    let exit_code = status.code();

    if !status.success() {
        return Err(AppError::SvnCommand {
            message: format!("svn {} 执行失败", args.first().copied().unwrap_or("")),
            stderr,
            exit_code,
        });
    }

    Ok(SvnOutput { stdout, stderr })
}

/// 检测 svn 是否可用，返回 svn --version 第一行
pub fn check_svn_version(svn_bin: &str) -> AppResult<String> {
    let out = run_svn(svn_bin, &["--version", "--quiet"])?;
    Ok(out.stdout.trim().to_string())
}
