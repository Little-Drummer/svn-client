use std::process::{Command, Stdio};

use encoding_rs::{GBK, UTF_8};

use crate::errors::{AppError, AppResult};

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

/// 一次性执行命令，等待结束后返回输出
pub fn run_svn(svn_bin: &str, args: &[&str]) -> AppResult<SvnOutput> {
    let mut cmd = build_command(svn_bin, args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::SvnNotFound(format!("找不到可执行文件 {}: {}", svn_bin, e))
        } else {
            AppError::Io(e)
        }
    })?;

    let stdout = decode_output(&output.stdout);
    let stderr = decode_output(&output.stderr);
    let exit_code = output.status.code();

    if !output.status.success() {
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
