import type { ClassValue } from "clsx"
import { clsx } from "clsx"
import { twMerge } from "tailwind-merge"

import type { WorkingCopyEntry } from '../types/svn'

// ===== 工作副本智能标签推断（本地路径结构优先 + SVN 分支信息辅助） =====
// 用于左侧列表、面包屑等处自动区分大量同名模块（rest/front 等）和个人分支

export type WorkingCopyTreeSegmentKind = 'project' | 'environment' | 'module' | 'repository'

export interface WorkingCopyTreeSegment {
  key: string
  label: string
  value: string
  kind: WorkingCopyTreeSegmentKind
  title?: string
}

const LOCAL_WORK_ROOT = 'work'
const ENV_FOLDERS = new Set(['develop', 'test', 'produce'])
const MODULE_FOLDERS = new Set([
  'front',
  'rest',
  'database',
  'updatesql',
  'xxl-job-admin',
  'doc-archive',
  'ai-appraisal',
])

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

function safeDecode(segment: string): string {
  try {
    return decodeURIComponent(segment)
  } catch {
    return segment
  }
}

function pathParts(path: string): string[] {
  return path.split(/[\\/]/).filter(Boolean)
}

function localWorkTail(path: string): string[] | null {
  const parts = pathParts(path)
  const workIdx = parts.findIndex((p) => p.toLowerCase() === LOCAL_WORK_ROOT)
  if (workIdx >= 0 && workIdx + 1 < parts.length) {
    return parts.slice(workIdx + 1)
  }
  return null
}

// 解码 SVN 返回的 URL / relativeUrl（XML 中非 ASCII 路径会被 percent-encode），用于 hover title 等人类可读展示
export function getDecodedUrl(u?: string | null): string {
  if (!u) return ''
  try {
    const url = new URL(u)
    const decodedPath = url.pathname.split('/').map(safeDecode).join('/')
    return `${url.origin}${decodedPath}${url.search}${url.hash}`
  } catch {
    // 非标准 URL 时按路径段解码（兼容 ^/branches/xxx 形式）
    return u.split('/').map(safeDecode).join('/')
  }
}

export function getDecodedBranchInfo(wc: WorkingCopyEntry): string {
  if (wc.relativeUrl) {
    return wc.relativeUrl.split('/').map(safeDecode).join('/')
  }
  if (wc.url) {
    return getDecodedUrl(wc.url)
  }
  return ''
}

export function getLocalProject(path: string): string | null {
  const tail = localWorkTail(path)
  if (tail?.[0]) {
    return tail[0]
  }
  const parts = pathParts(path)
  if (parts.length >= 3) {
    const cand = parts[parts.length - 3]
    const lower = cand.toLowerCase()
    if (!['develop', 'produce', 'test', 'database', 'front', 'rest', 'updatesql'].includes(lower)) {
      return cand
    }
  }
  return null
}

export function getWorkingCopyLeafLabel(wc: WorkingCopyEntry): string {
  if (wc.displayName) return wc.displayName
  const parts = pathParts(wc.path)
  return parts[parts.length - 1] || wc.path
}

export function getWorkingCopyTreePath(wc: WorkingCopyEntry): WorkingCopyTreeSegment[] {
  const tail = localWorkTail(wc.path)
  if (tail?.[0]) {
    const project = tail[0]
    const segments: WorkingCopyTreeSegment[] = [
      {
        key: `local:${project}`,
        label: project,
        value: project,
        kind: 'project',
      },
    ]

    const second = tail[1]
    const third = tail[2]
    const secondLower = second?.toLowerCase() ?? ''

    // 本地工作目录通常是 项目 / 环境或分支 / 模块；只有模块直接挂在项目根下才补“默认”。
    if (second && ENV_FOLDERS.has(secondLower)) {
      segments.push({
        key: `local:${project}/${second}`,
        label: second,
        value: second,
        kind: 'environment',
      })
      if (third) {
        segments.push({
          key: `local:${project}/${second}/${third}`,
          label: third,
          value: third,
          kind: 'module',
        })
      }
      return segments
    }

    if (second) {
      if (third) {
        segments.push({
          key: `local:${project}/${second}`,
          label: second,
          value: second,
          kind: 'environment',
        })
        segments.push({
          key: `local:${project}/${second}/${third}`,
          label: third,
          value: third,
          kind: 'module',
        })
        return segments
      }

      segments.push({
        key: `local:${project}/默认`,
        label: '默认',
        value: '默认',
        kind: 'environment',
      })
      if (tail.length >= 2) {
        segments.push({
          key: `local:${project}/默认/${second}`,
          label: second,
          value: second,
          kind: 'module',
        })
      }
    }
    return segments
  }

  const root = wc.repositoryRoot ?? wc.url ?? '未知远端'
  return [
    {
      key: `repo:${root}`,
      label: root,
      value: root,
      kind: 'repository',
      title: getDecodedUrl(root),
    },
  ]
}

export function getBranchFromRelative(rel?: string | null): string | null {
  if (!rel) return null
  const decoded = rel.split('/').map(safeDecode).join('/')
  const m = decoded.match(/branches\/([^/]+)/i) || decoded.match(/tags\/([^/]+)/i)
  return m ? m[1] : null
}

export function getSmartLabel(wc: WorkingCopyEntry): string {
  if (wc.displayName) return wc.displayName

  const project = getLocalProject(wc.path)
  const parts = pathParts(wc.path)
  const leaf = parts[parts.length - 1] || wc.path

  const branch = getBranchFromRelative(wc.relativeUrl)

  if (branch) {
    if (project) {
      const module = leaf !== branch ? ` (${leaf})` : ''
      return `${project} ${branch}${module}`
    }
    return branch
  }

  if (project) {
    const projIdx = parts.lastIndexOf(project)
    const tail = projIdx >= 0 ? parts.slice(projIdx + 1).join(' / ') : leaf
    return `${project} ${tail}`
  }

  return leaf
}

export function getSmartSubtitle(wc: WorkingCopyEntry): string {
  const rel = wc.relativeUrl ? wc.relativeUrl.split('/').map(safeDecode).join('/') : ''
  const shortRel = rel ? rel.replace(/^\^?\/+/, '') : ''
  const relParts = shortRel.split('/').filter(Boolean)
  const relTail = relParts.slice(-2).join('/')
  if (relTail) return relTail
  if (wc.url) {
    try {
      return new URL(wc.url).pathname.split('/').filter(Boolean).slice(-2).join('/')
    } catch {
      return wc.url.split('/').slice(-2).join('/')
    }
  }
  return ''
}

export function getGroupKey(wc: WorkingCopyEntry): string {
  const proj = getLocalProject(wc.path)
  if (proj) return `local:${proj}`
  return `repo:${wc.repositoryRoot ?? wc.url ?? '未知远端'}`
}

export function getFullTitle(wc: WorkingCopyEntry): string {
  const loc = wc.path
  const branchInfo = getDecodedBranchInfo(wc)
  return branchInfo ? `${loc}\n${branchInfo}` : loc
}
