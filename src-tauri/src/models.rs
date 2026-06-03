use serde::{Deserialize, Serialize};

// 对外暴露的工作副本元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCopyEntry {
    pub id: String,
    pub path: String,
    pub url: Option<String>,
    pub repository_root: Option<String>,
    pub revision: Option<u64>,
    pub last_seen_at: Option<String>,
    #[serde(default)]
    pub relative_url: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
}

// 保存的远端仓库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryEntry {
    pub id: String,
    pub name: String,
    pub url: String,
    pub username: Option<String>,
    pub last_accessed_at: Option<String>,
}

// svn list --xml 远端目录项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteListEntry {
    pub name: String,
    pub path: String,
    pub url: String,
    pub kind: String,
    pub size: Option<u64>,
    pub revision: Option<u64>,
    pub author: Option<String>,
    pub date: Option<String>,
}

// 本地工作副本文件树节点
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCopyFileEntry {
    pub name: String,
    pub path: String,
    pub relative_path: String,
    pub kind: String,
    pub children: Vec<WorkingCopyFileEntry>,
}

// svn info --xml 解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SvnInfo {
    pub path: String,
    pub url: String,
    pub repository_root: String,
    pub repository_uuid: Option<String>,
    pub revision: u64,
    pub kind: String,
    pub relative_url: Option<String>,
    pub last_changed_revision: Option<u64>,
    pub last_changed_author: Option<String>,
    pub last_changed_date: Option<String>,
}

// svn status --xml 单个文件项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SvnStatusEntry {
    pub path: String,
    pub item: String, // modified/added/deleted/normal/unversioned/conflicted/missing/...
    pub props: Option<String>,
    pub copied: bool,
    pub revision: Option<u64>,
    pub commit_revision: Option<u64>,
    pub commit_author: Option<String>,
    pub commit_date: Option<String>,
}

// svn log 单条提交
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SvnLogEntry {
    pub revision: u64,
    pub author: Option<String>,
    pub date: Option<String>,
    pub message: Option<String>,
    pub paths: Vec<SvnLogPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SvnLogPath {
    pub path: String,
    pub action: String, // A/M/D/R
    pub kind: Option<String>,
    pub copyfrom_path: Option<String>,
    pub copyfrom_rev: Option<u64>,
}

// 流式 status 事件载荷，按 request_id 区分不同次刷新
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum StatusStreamEvent {
    Entries {
        request_id: String,
        entries: Vec<SvnStatusEntry>,
    },
    Finished {
        request_id: String,
        count: usize,
    },
    Failed {
        request_id: String,
        message: String,
    },
}

// 长任务事件载荷
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum TaskEvent {
    Started {
        task_id: String,
    },
    Stdout {
        task_id: String,
        line: String,
    },
    Stderr {
        task_id: String,
        line: String,
    },
    Finished {
        task_id: String,
        success: bool,
        exit_code: Option<i32>,
    },
}
