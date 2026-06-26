use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::thread;

use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::{StatusStreamEvent, SvnStatusEntry, TaskEvent};
use crate::svn::merge::{run_merge_flow, MergeRoute};
use crate::svn::status::svn_status_streaming;
use crate::svn::streaming::{run_svn_streaming_cancellable, StreamLine};

pub const TASK_EVENT_NAME: &str = "svn-task";
pub const STATUS_EVENT_NAME: &str = "svn-status-stream";

/// 运行中 svn 长任务的进程注册表，按 task_id 持有子进程句柄，支持外部终止。
/// 作为 Tauri 托管状态在命令间共享；内部用 Arc 以便克隆进后台线程。
#[derive(Clone, Default)]
pub struct ProcessRegistry {
    inner: Arc<Mutex<HashMap<String, ProcEntry>>>,
}

struct ProcEntry {
    child: Arc<Mutex<Child>>,
    canceled: bool,
}

impl ProcessRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn register(&self, task_id: &str, child: Arc<Mutex<Child>>) {
        self.inner.lock().unwrap().insert(
            task_id.to_string(),
            ProcEntry {
                child,
                canceled: false,
            },
        );
    }

    /// 标记取消并 kill 子进程。返回 task_id 是否存在（即任务是否还在运行）。
    pub fn cancel(&self, task_id: &str) -> bool {
        let mut map = self.inner.lock().unwrap();
        match map.get_mut(task_id) {
            Some(entry) => {
                entry.canceled = true;
                // 进程可能恰好已自然退出，kill 报错可忽略
                let _ = entry.child.lock().unwrap().kill();
                true
            }
            None => false,
        }
    }

    /// 任务结束时注销，返回它是否曾被用户主动取消。
    fn finish(&self, task_id: &str) -> bool {
        self.inner
            .lock()
            .unwrap()
            .remove(task_id)
            .map(|e| e.canceled)
            .unwrap_or(false)
    }
}

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
    registry: ProcessRegistry,
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
        let reg_for_spawn = registry.clone();
        let task_id_for_spawn = task_id_thread.clone();

        let result = run_svn_streaming_cancellable(
            &svn_bin,
            &arg_refs,
            |child| reg_for_spawn.register(&task_id_for_spawn, child),
            |line| {
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
            },
        );

        // 注销并取回「是否被用户终止」标记
        let canceled = registry.finish(&task_id_thread);

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
                canceled,
            },
        );

        // 任务结束发系统通知，长任务跑完时用户可能已切到别的窗口
        let body = if canceled {
            format!("{} 已终止", label)
        } else if success {
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

/// 启动一个合并长任务（update→merge→commit + 可选搁置/恢复），立即返回 task_id。
/// 过程输出复用 svn-task 事件，前端用现有 TaskOutput 组件展示。
pub fn spawn_merge_task(
    app: AppHandle,
    svn_bin: String,
    route: MergeRoute,
    revisions: Vec<u64>,
    message: String,
    shelves_dir: PathBuf,
) -> AppResult<String> {
    let task_id = Uuid::new_v4().to_string();
    let app_thread = app.clone();

    let _ = app.emit(
        TASK_EVENT_NAME,
        TaskEvent::Started {
            task_id: task_id.clone(),
        },
    );

    let task_id_thread = task_id.clone();
    thread::spawn(move || {
        let tid = task_id_thread.clone();
        let app_lines = app_thread.clone();
        let success = run_merge_flow(
            &svn_bin,
            &route,
            &revisions,
            &message,
            &shelves_dir,
            |line| {
                let event = match line {
                    StreamLine::Stdout(s) => TaskEvent::Stdout {
                        task_id: tid.clone(),
                        line: s,
                    },
                    StreamLine::Stderr(s) => TaskEvent::Stderr {
                        task_id: tid.clone(),
                        line: s,
                    },
                };
                let _ = app_lines.emit(TASK_EVENT_NAME, event);
            },
        );

        let _ = app_thread.emit(
            TASK_EVENT_NAME,
            TaskEvent::Finished {
                task_id: task_id_thread,
                success,
                exit_code: None,
                canceled: false,
            },
        );

        let body = if success {
            "合并 完成".to_string()
        } else {
            "合并 失败".to_string()
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
