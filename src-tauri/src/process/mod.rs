use std::sync::Arc;
use std::thread;

use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::TaskEvent;
use crate::svn::streaming::{run_svn_streaming, StreamLine};

pub const TASK_EVENT_NAME: &str = "svn-task";

/// 启动一个 svn 长任务，立即返回 task_id；任务过程通过 svn-task 事件推送到前端。
/// args 必须是已经准备好的 svn 子命令（不含 svn 本身），例如 ["update", "--non-interactive", path]
pub fn spawn_svn_task(app: AppHandle, svn_bin: String, args: Vec<String>) -> AppResult<String> {
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
    });

    Ok(task_id)
}
