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
  | { kind: 'finished'; taskId: string; success: boolean; exitCode?: number | null }

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
