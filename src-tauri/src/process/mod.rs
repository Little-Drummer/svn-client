use std::sync::Arc;
use std::thread;

use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::{StatusStreamEvent, SvnStatusEntry, TaskEvent};
use crate::svn::status::svn_status_streaming;
use crate::svn::streaming::{run_svn_streaming, StreamLine};

pub const TASK_EVENT_NAME: &str = "svn-task";
pub const STATUS_EVENT_NAME: &str = "svn-status-stream";

/// 流式刷新工作副本状态，立即返回 request_id；entry 分批通过 svn-status-stream 事件推送。
/// 前端按 request_id 区分，丢弃过期请求的批次。
pub fn spawn_status_stream(
    app: AppHandle,
    svn_bin: String,
    target: String,
    show_unversioned: bool,
) -> String {
    let request_id = Uuid::new_v4().to_string();
    let rid = request_id.clone();

    thread::spawn(move || {
        // 攒批后再 emit，降低大工作副本下的事件风暴
        const BATCH: usize = 32;
        let mut batch: Vec<SvnStatusEntry> = Vec::with_capacity(BATCH);
        let mut total: usize = 0;

        let result = svn_status_streaming(&svn_bin, &target, show_unversioned, |entry| {
            batch.push(entry);
            total += 1;
            if batch.len() >= BATCH {
                let _ = app.emit(
                    STATUS_EVENT_NAME,
                    StatusStreamEvent::Entries {
                        request_id: rid.clone(),
                        entries: std::mem::take(&mut batch),
                    },
                );
            }
        });

        if !batch.is_empty() {
            let _ = app.emit(
                STATUS_EVENT_NAME,
                StatusStreamEvent::Entries {
                    request_id: rid.clone(),
                    entries: std::mem::take(&mut batch),
                },
            );
        }

        match result {
            Ok(()) => {
                let _ = app.emit(
                    STATUS_EVENT_NAME,
                    StatusStreamEvent::Finished {
                        request_id: rid.clone(),
                        count: total,
                    },
                );
            }
            Err(e) => {
                let _ = app.emit(
                    STATUS_EVENT_NAME,
                    StatusStreamEvent::Failed {
                        request_id: rid.clone(),
                        message: e.to_string(),
                    },
                );
            }
        }
    });

    request_id
}

/// 启动一个 svn 长任务，立即返回 task_id；任务过程通过 svn-task 事件推送到前端。
/// args 必须是已经准备好的 svn 子命令（不含 svn 本身），例如 ["update", "--non-interactive", path]
/// label 用于任务完成时的系统通知标题（如「提交」「更新」「检出」）
pub fn spawn_svn_task(
    app: AppHandle,
    svn_bin: String,
    args: Vec<String>,
    label: String,
) -> AppResult<String> {
    let task_id = Uuid::new_v4().to_string();
    let task_id_thread = task_id.clone();
    let app_thread = app.clone();
    let app_started = app.clone();

    // 触发 started 事件
    let _ = app_started.emit(
        TASK_EVENT_NAME,
        TaskEvent::Started {
            task_id: task_id.clone(),
        },
    );

    let args = Arc::new(args);

    thread::spawn(move || {
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let task_id_for_lines = task_id_thread.clone();
        let app_for_lines = app_thread.clone();

        let result = run_svn_streaming(&svn_bin, &arg_refs, |line| {
            let event = match line {
                StreamLine::Stdout(s) => TaskEvent::Stdout {
                    task_id: task_id_for_lines.clone(),
                    line: s,
                },
                StreamLine::Stderr(s) => TaskEvent::Stderr {
                    task_id: task_id_for_lines.clone(),
                    line: s,
                },
            };
            let _ = app_for_lines.emit(TASK_EVENT_NAME, event);
        });

        let (success, exit_code) = match result {
            Ok(r) => (r.success, r.exit_code),
            Err(e) => {
                // 命令直接启动失败也走 stderr 事件，便于前端展示原因
                let msg = match &e {
                    AppError::SvnNotFound(m) => m.clone(),
                    other => other.to_string(),
                };
                let _ = app_thread.emit(
                    TASK_EVENT_NAME,
                    TaskEvent::Stderr {
                        task_id: task_id_thread.clone(),
                        line: msg,
                    },
                );
                (false, None)
            }
        };

        let _ = app_thread.emit(
            TASK_EVENT_NAME,
            TaskEvent::Finished {
                task_id: task_id_thread,
                success,
                exit_code,
            },
        );

        // 任务结束发系统通知，长任务跑完时用户可能已切到别的窗口
        let body = if success {
            format!("{} 完成", label)
        } else {
            match exit_code {
                Some(code) => format!("{} 失败（退出码 {}）", label, code),
                None => format!("{} 失败", label),
            }
        };
        let _ = app_thread
            .notification()
            .builder()
            .title("SVN Client")
            .body(body)
            .show();
    });

    Ok(task_id)
}
