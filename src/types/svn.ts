// 与后端 models.rs 对应的类型定义。后端 serde 用 camelCase 序列化。

export interface WorkingCopyEntry {
  id: string
  path: string
  url?: string | null
  repositoryRoot?: string | null
  revision?: number | null
  lastSeenAt?: string | null
  relativeUrl?: string | null
  displayName?: string | null
  // 路径是否存在（外置卷未挂载/目录被删时为 false），由后端列表时实时计算
  available?: boolean
}

// 项目分组视图，与后端 Project 对应。由扁平工作副本实时聚合，不持久化。
export interface Project {
  name: string
  branches: ProjectBranch[]
}

export interface ProjectBranch {
  environment: string // develop / test / produce / 默认
  modules: ProjectModule[]
}

export interface ProjectModule {
  module: string // rest / database / updatesql / front / 个人分支名
  workingCopyId: string
  path: string
  url?: string | null
}

// 一条合并方向，与后端 MergeRoute 对应
export interface MergeRoute {
  id: string
  name: string
  sourceLabel: string
  sourcePath: string
  targetPath: string
  kind: string
  personalBranch?: string | null
  syncBranch: boolean
}

export interface MergeRouteConfig {
  id: string
  projectName: string
  name: string
  sourceEnv: string
  sourceModule: string
  targetEnv: string
  targetModule: string
  enabled: boolean
}

// 候选合并版本，与后端 MergeRevision 对应
export interface MergeRevision {
  revision: number
  author?: string | null
  date?: string | null
  message: string
}

export interface MergePreview {
  command: string
  message: string
}

// 本地开发配置预设，与后端 ConfigPreset 对应。全局统一，不绑定项目。
export interface PresetFragment {
  startLine: number // 1-based
  endLine: number
  lines: string[]
  contextBefore?: string[]
  contextAfter?: string[]
}

export interface PresetFile {
  relPath: string
  content: string
  // 非空表示片段模式：应用时只替换这些行
  fragments?: PresetFragment[]
}

export interface ConfigPreset {
  id: string
  projectName?: string | null
  name: string
  files: PresetFile[]
}

// 捕获预设时提交的文件说明，ranges 为空表示整文件
export interface CapturePresetFile {
  path: string
  ranges: [number, number][]
}

// 应用/预览预设时单个文件的计划与结果
export interface PresetApplyPlan {
  relPath: string
  action: 'create' | 'overwrite' | 'patch' | 'unchanged' | 'conflict'
  detail: string
  oldLines: string[]
  newLines: string[]
}

// 增量打包，与后端 package.rs 对应
export interface PackageOptions {
  requirementName: string
  requirementDesc: string
  hasDb: boolean
  hasUrl: boolean
}

export interface PackageRevision {
  revision: number
  author?: string | null
  date?: string | null
  message: string
}

export interface PackageBuildResult {
  baseDir: string
  frontDir: string
  finalRestDir: string
  version: string
  copiedCount: number
  notFound: string[]
  log: string[]
}

export interface PackageZipResult {
  zipPath: string
  size: number
}

export interface RepositoryEntry {
  id: string
  name: string
  url: string
  username?: string | null
  lastAccessedAt?: string | null
}

export interface RemoteListEntry {
  name: string
  path: string
  url: string
  kind: 'dir' | 'file' | string
  size?: number | null
  revision?: number | null
  author?: string | null
  date?: string | null
}

export interface WorkingCopyFileEntry {
  name: string
  path: string
  relativePath: string
  kind: 'dir' | 'file' | string
  size?: number | null
  modifiedAt?: string | null
  svnItem?: SvnStatusItem | null
  props?: string | null
  copied: boolean
  revision?: number | null
  commitRevision?: number | null
  commitAuthor?: string | null
  commitDate?: string | null
  children: WorkingCopyFileEntry[]
}

export interface SvnInfo {
  path: string
  url: string
  repositoryRoot: string
  repositoryUuid?: string | null
  revision: number
  kind: string
  relativeUrl?: string | null
  lastChangedRevision?: number | null
  lastChangedAuthor?: string | null
  lastChangedDate?: string | null
}

export type SvnStatusItem =
  | 'modified'
  | 'added'
  | 'deleted'
  | 'replaced'
  | 'conflicted'
  | 'missing'
  | 'unversioned'
  | 'ignored'
  | 'external'
  | 'incomplete'
  | 'obstructed'
  | 'normal'
  | string

export interface SvnStatusEntry {
  path: string
  item: SvnStatusItem
  props?: string | null
  copied: boolean
  revision?: number | null
  commitRevision?: number | null
  commitAuthor?: string | null
  commitDate?: string | null
}

export interface SvnLogPath {
  path: string
  action: string
  kind?: string | null
  copyfromPath?: string | null
  copyfromRev?: number | null
}

export interface SvnLogEntry {
  revision: number
  author?: string | null
  date?: string | null
  message?: string | null
  paths: SvnLogPath[]
}

// 日志视图的目标：本地工作副本 或 远端 URL，统一驱动主区域 LogView
export interface LogTarget {
  kind: 'wc' | 'remote'
  // 给 svn log 的目标：wc 传本地路径，remote 传 URL
  target: string
  // 头部展示用的标题
  title: string
  // 仓库根 URL，用于把 log 的 repo-root-relative path 拼成完整文件 URL 做单文件 diff
  repositoryRoot?: string | null
  // 当前工作副本所在版本（仅 wc 有），用于在列表里标记
  currentRevision?: number | null
}

// 流式 status 事件，与后端 StatusStreamEvent 对应
export type StatusStreamEvent =
  | { kind: 'entries'; requestId: string; entries: SvnStatusEntry[] }
  | { kind: 'finished'; requestId: string; count: number }
  | { kind: 'failed'; requestId: string; message: string }

// 长任务事件，与后端 TaskEvent 对应
export type TaskEvent =
  | { kind: 'started'; taskId: string }
  | { kind: 'stdout'; taskId: string; line: string }
  | { kind: 'stderr'; taskId: string; line: string }
  | {
      kind: 'finished'
      taskId: string
      success: boolean
      exitCode?: number | null
      canceled?: boolean
    }

// 错误类型
export type AppErrorPayload =
  | { kind: 'svn_command'; message: string; stderr: string; exit_code?: number | null }
  | { kind: 'svn_not_found'; message: string }
  | { kind: 'xml_parse'; message: string }
  | { kind: 'io'; message: string }
  | { kind: 'json'; message: string }
  | { kind: 'invalid_path'; message: string }
  | { kind: 'not_working_copy'; message: string }
  | { kind: 'task_not_found'; message: string }
  | { kind: 'other'; message: string }
