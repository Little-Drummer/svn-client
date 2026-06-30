import { invoke } from '@tauri-apps/api/core'

import type {
  Project,
  MergeRoute,
  MergeRevision,
  MergePreview,
  PackageOptions,
  PackageRevision,
  PackageBuildResult,
  PackageZipResult,
  CapturePresetFile,
  ConfigPreset,
  PresetApplyPlan,
  SvnInfo,
  SvnLogEntry,
  SvnStatusEntry,
  RepositoryEntry,
  RemoteListEntry,
  WorkingCopyEntry,
  WorkingCopyFileEntry,
} from '../types/svn'

export const api = {
  // 环境
  checkEnvironment: () => invoke<string>('svn_check_environment'),
  getSvnBin: () => invoke<string>('get_svn_bin'),
  setSvnBin: (bin: string | null) => invoke<void>('set_svn_bin', { bin }),

  // 远端仓库
  listRepositories: () => invoke<RepositoryEntry[]>('list_repositories'),
  saveRepository: (params: {
    id?: string
    name: string
    url: string
    username?: string
  }) => invoke<RepositoryEntry>('save_repository', params),
  removeRepository: (id: string) => invoke<void>('remove_repository', { id }),
  testRepositoryConnection: (params: { id?: string; url: string; username?: string }) =>
    invoke<SvnInfo>('test_repository_connection', params),
  listRemote: (params: { url: string; username?: string }) =>
    invoke<RemoteListEntry[]>('svn_list_remote', params),
  catRemote: (params: { url: string; username?: string }) =>
    invoke<string>('svn_cat_remote', params),

  // 工作副本
  listWorkingCopies: () => invoke<WorkingCopyEntry[]>('list_working_copies'),
  addWorkingCopy: (path: string) => invoke<WorkingCopyEntry>('add_working_copy', { path }),
  removeWorkingCopy: (id: string) => invoke<void>('remove_working_copy', { id }),
  refreshWorkingCopy: (id: string) =>
    invoke<WorkingCopyEntry>('refresh_working_copy', { id }),
  setWorkingCopyDisplayName: (id: string, displayName: string | null) =>
    invoke<WorkingCopyEntry>('set_working_copy_display_name', { id, displayName }),
  listProjects: () => invoke<Project[]>('list_projects'),
  scanAndAddProject: (path: string) =>
    invoke<WorkingCopyEntry[]>('scan_and_add_project', { path }),

  // 多级合并
  mergeListRoutes: (projectName: string) =>
    invoke<MergeRoute[]>('merge_list_routes', { projectName }),
  mergeFetchRevisions: (route: MergeRoute) =>
    invoke<MergeRevision[]>('merge_fetch_revisions', { route }),
  mergePreview: (route: MergeRoute, entries: MergeRevision[], revisions: number[]) =>
    invoke<MergePreview>('merge_preview', { route, entries, revisions }),
  mergeExecute: (route: MergeRoute, revisions: number[], message: string) =>
    invoke<string>('merge_execute', { route, revisions, message }),

  // 增量打包
  packageFetchRevisions: (restPath: string, limit?: number) =>
    invoke<PackageRevision[]>('package_fetch_revisions', { restPath, limit }),
  packageBuild: (restPath: string, options: PackageOptions, revisions: number[]) =>
    invoke<PackageBuildResult>('package_build', { restPath, options, revisions }),
  packageMakeZip: (baseDir: string, requirementName: string) =>
    invoke<PackageZipResult>('package_make_zip', { baseDir, requirementName }),
  packageCommitVersion: (restPath: string, version: string) =>
    invoke<string>('package_commit_version', { restPath, version }),

  // 本地开发配置预设（全局统一）
  listConfigPresets: () => invoke<ConfigPreset[]>('list_config_presets'),
  captureConfigPreset: (name: string, wcRoot: string, files: CapturePresetFile[]) =>
    invoke<ConfigPreset>('capture_config_preset', { name, wcRoot, files }),
  previewConfigPreset: (id: string, wcRoot: string) =>
    invoke<PresetApplyPlan[]>('preview_config_preset', { id, wcRoot }),
  applyConfigPreset: (id: string, wcRoot: string) =>
    invoke<PresetApplyPlan[]>('apply_config_preset', { id, wcRoot }),
  deleteConfigPreset: (id: string) => invoke<void>('delete_config_preset', { id }),
  listWorkingCopyFiles: (root: string) =>
    invoke<WorkingCopyFileEntry[]>('list_working_copy_files', { root }),
  createWorkingCopyFolder: (parentPath: string, name: string) =>
    invoke<string>('create_working_copy_folder', { parentPath, name }),

  // 查询
  info: (path: string) => invoke<SvnInfo>('svn_get_info', { path }),
  status: (path: string, showUnversioned = true) =>
    invoke<SvnStatusEntry[]>('svn_get_status', { path, showUnversioned }),
  statusStream: (path: string, showUnversioned = true, requestId: string) =>
    invoke<string>('svn_get_status_stream', { path, showUnversioned, requestId }),
  log: (params: {
    path: string
    limit?: number
    revisionRange?: string
    search?: string
    author?: string
    dateFrom?: string
    dateTo?: string
    withPaths?: boolean
  }) => invoke<SvnLogEntry[]>('svn_get_log', params),
  diff: (path: string) => invoke<string>('svn_get_diff', { path }),
  diffRevision: (path: string, revision: number) =>
    invoke<string>('svn_get_diff_revision', { path, revision }),
  baseContent: (path: string) => invoke<string>('svn_get_base_content', { path }),
  catRevision: (target: string, revision: number) =>
    invoke<string>('svn_get_cat_revision', { target, revision }),
  readFileText: (path: string) => invoke<string>('read_file_text', { path }),
  revealInFileManager: (path: string) => invoke<void>('reveal_in_file_manager', { path }),
  openInTerminal: (path: string) => invoke<void>('open_in_terminal', { path }),
  revert: (paths: string[]) => invoke<void>('svn_revert', { paths }),
  add: (paths: string[]) => invoke<void>('svn_add', { paths }),
  delete: (paths: string[]) => invoke<void>('svn_delete', { paths }),
  ignore: (paths: string[]) => invoke<void>('svn_ignore', { paths }),

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
  // 终止运行中的长任务；返回该任务当时是否仍在运行
  cancelTask: (taskId: string) => invoke<boolean>('svn_cancel_task', { taskId }),
  // 清理工作副本残留锁（终止任务后 E155004），完成后才返回
  cleanup: (path: string) => invoke<void>('svn_cleanup', { path }),
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
