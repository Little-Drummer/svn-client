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
    // 列表返回时实时计算：路径是否存在（外置卷未挂载/目录被删时为 false），不参与持久化语义
    #[serde(default = "default_true")]
    pub available: bool,
}

fn default_true() -> bool {
    true
}

// 项目分组视图：把扁平工作副本聚合成 项目 → 环境(分支) → 模块 结构。
// 仅用于对外展示和供合并/打包定位各分支，不持久化（每次由工作副本列表实时计算）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub name: String,
    pub branches: Vec<ProjectBranch>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBranch {
    pub environment: String, // develop / test / produce / 默认
    pub modules: Vec<ProjectModule>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModule {
    pub module: String, // rest / database / updatesql / front / 个人分支名
    pub working_copy_id: String,
    pub path: String,
    pub url: Option<String>,
}

// 项目级自定义合并方向。默认方向仍由项目结构自动推断，这里只补充特殊流转。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeRouteConfig {
    pub id: String,
    pub project_name: String,
    pub name: String,
    pub source_env: String,
    pub source_module: String,
    pub target_env: String,
    pub target_module: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

// 本地开发配置预设：保存一组文件的「本地开发版本」内容（整文件或若干行片段），
// 全局统一维护，不绑定项目，新项目可直接套用；拉取/切换分支后一键套回。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPreset {
    pub id: String,
    // 旧版本按项目绑定，现仅作为来源标记保留，列表不再按它过滤
    #[serde(default)]
    pub project_name: Option<String>,
    pub name: String,
    pub files: Vec<PresetFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetFile {
    pub rel_path: String, // 相对捕获时工作副本根目录
    pub content: String,  // 捕获时的完整文件内容（片段模式下也保留，便于回看与锚点回退）
    // 非空表示片段模式：应用时只替换这些行，而不是整文件覆盖
    #[serde(default)]
    pub fragments: Vec<PresetFragment>,
}

// 一个行片段：记录捕获时的行号与内容，并带上前后几行作为定位锚点，
// 目标文件行号漂移时仍能找到正确位置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetFragment {
    pub start_line: usize, // 1-based，捕获时片段在源文件中的位置
    pub end_line: usize,
    pub lines: Vec<String>,
    #[serde(default)]
    pub context_before: Vec<String>,
    #[serde(default)]
    pub context_after: Vec<String>,
}

// 捕获预设时前端提交的单个文件说明：ranges 为空表示整文件
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturePresetFile {
    pub path: String,
    #[serde(default)]
    pub ranges: Vec<[usize; 2]>, // 1-based 闭区间 [start, end]
}

// 应用（或预览）预设时单个文件的计划/结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetApplyPlan {
    pub rel_path: String,
    // create=新建文件 overwrite=整文件覆盖 patch=行片段替换 unchanged=内容已一致 conflict=找不到落点
    pub action: String,
    pub detail: String,
    // patch 模式下展示给用户确认的变更：目标文件中将被替换掉的行 → 预设写入的行
    pub old_lines: Vec<String>,
    pub new_lines: Vec<String>,
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
    pub size: Option<u64>,
    pub modified_at: Option<String>,
    pub svn_item: Option<String>,
    pub props: Option<String>,
    pub copied: bool,
    pub revision: Option<u64>,
    pub commit_revision: Option<u64>,
    pub commit_author: Option<String>,
    pub commit_date: Option<String>,
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
    pub repos_item: Option<String>,
    pub repos_props: Option<String>,
}

// 工作副本侧栏使用的轻量状态统计，避免前端重复解析完整状态列表。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCopyStatusSummary {
    pub uncommitted: usize,
    pub outdated: usize,
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
// rename_all 只改变体名，变体内字段要靠 rename_all_fields 才会变 camelCase（前端按 camelCase 读取）
#[derive(Debug, Clone, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
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

// 长任务事件载荷，字段命名同样依赖 rename_all_fields 与前端对齐
#[derive(Debug, Clone, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
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
        // 是否由用户主动终止，前端据此区分「已终止」与普通失败
        #[serde(default)]
        canceled: bool,
    },
}
