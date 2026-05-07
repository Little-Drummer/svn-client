use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("svn 命令执行失败: {message}")]
    SvnCommand {
        message: String,
        stderr: String,
        exit_code: Option<i32>,
    },

    #[error("svn 未安装或不在 PATH 中: {0}")]
    SvnNotFound(String),

    #[error("XML 解析失败: {0}")]
    XmlParse(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON 序列化失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("非法路径: {0}")]
    InvalidPath(String),

    #[error("路径不是 SVN 工作副本: {0}")]
    NotWorkingCopy(String),

    #[error("任务不存在: {0}")]
    TaskNotFound(String),

    #[error("{0}")]
    Other(String),
}

// 统一序列化为前端可读的结构体，前端可以拿到错误类型再走分支
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppErrorPayload {
    SvnCommand {
        message: String,
        stderr: String,
        exit_code: Option<i32>,
    },
    SvnNotFound {
        message: String,
    },
    XmlParse {
        message: String,
    },
    Io {
        message: String,
    },
    Json {
        message: String,
    },
    InvalidPath {
        message: String,
    },
    NotWorkingCopy {
        message: String,
    },
    TaskNotFound {
        message: String,
    },
    Other {
        message: String,
    },
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let payload = match self {
            AppError::SvnCommand {
                message,
                stderr,
                exit_code,
            } => AppErrorPayload::SvnCommand {
                message: message.clone(),
                stderr: stderr.clone(),
                exit_code: *exit_code,
            },
            AppError::SvnNotFound(m) => AppErrorPayload::SvnNotFound { message: m.clone() },
            AppError::XmlParse(m) => AppErrorPayload::XmlParse { message: m.clone() },
            AppError::Io(e) => AppErrorPayload::Io { message: e.to_string() },
            AppError::Json(e) => AppErrorPayload::Json { message: e.to_string() },
            AppError::InvalidPath(m) => AppErrorPayload::InvalidPath { message: m.clone() },
            AppError::NotWorkingCopy(m) => AppErrorPayload::NotWorkingCopy { message: m.clone() },
            AppError::TaskNotFound(m) => AppErrorPayload::TaskNotFound { message: m.clone() },
            AppError::Other(m) => AppErrorPayload::Other { message: m.clone() },
        };
        payload.serialize(serializer)
    }
}

impl From<quick_xml::DeError> for AppError {
    fn from(e: quick_xml::DeError) -> Self {
        AppError::XmlParse(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
