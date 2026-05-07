import { invoke } from '@tauri-apps/api/core'

import type {
  SvnInfo,
  SvnLogEntry,
  SvnStatusEntry,
  WorkingCopyEntry,
} from '../types/svn'

export const api = {
  // 环境
  checkEnvironment: () => invoke<string>('svn_check_environment'),
  getSvnBin: () => invoke<string>('get_svn_bin'),
  setSvnBin: (bin: string | null) => invoke<void>('set_svn_bin', { bin }),

  // 工作副本
  listWorkingCopies: () => invoke<WorkingCopyEntry[]>('list_working_copies'),
  addWorkingCopy: (path: string) => invoke<WorkingCopyEntry>('add_working_copy', { path }),
  removeWorkingCopy: (id: string) => invoke<void>('remove_working_copy', { id }),
  refreshWorkingCopy: (id: string) =>
    invoke<WorkingCopyEntry>('refresh_working_copy', { id }),

  // 查询
  info: (path: string) => invoke<SvnInfo>('svn_get_info', { path }),
  status: (path: string, showUnversioned = true) =>
    invoke<SvnStatusEntry[]>('svn_get_status', { path, showUnversioned }),
  log: (params: {
    path: string
    limit?: number
    revisionRange?: string
    search?: string
    withPaths?: boolean
  }) => invoke<SvnLogEntry[]>('svn_get_log', params),
  diff: (path: string) => invoke<string>('svn_get_diff', { path }),
  diffRevision: (path: string, revision: number) =>
    invoke<string>('svn_get_diff_revision', { path, revision }),
  baseContent: (path: string) => invoke<string>('svn_get_base_content', { path }),
  readFileText: (path: string) => invoke<string>('read_file_text', { path }),
  revert: (paths: string[]) => invoke<void>('svn_revert', { paths }),

  // 长任务
  startCommit: (paths: string[], message: string) =>
    invoke<string>('svn_start_commit', { paths, message }),
  startUpdate: (path: string, revision?: string) =>
    invoke<string>('svn_start_update', { path, revision }),
  startCheckout: (params: {
    url: string
    targetPath: string
    revision?: string
    username?: string
    password?: string
  }) => invoke<string>('svn_start_checkout', params),
}

export function describeError(err: unknown): string {
  if (err && typeof err === 'object' && 'kind' in (err as Record<string, unknown>)) {
    const e = err as { kind: string; message?: string; stderr?: string }
    switch (e.kind) {
      case 'svn_command':
        return `命令失败：${e.message}\n${e.stderr ?? ''}`.trim()
      case 'svn_not_found':
        return `未找到 svn：${e.message}`
      case 'not_working_copy':
        return `不是 SVN 工作副本：${e.message}`
      case 'xml_parse':
        return `解析 svn 输出失败：${e.message}`
      default:
        return e.message ?? String(err)
    }
  }
  return typeof err === 'string' ? err : String(err)
}
