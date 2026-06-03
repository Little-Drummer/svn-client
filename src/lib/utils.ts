import type { ClassValue } from "clsx"
import { clsx } from "clsx"
import { twMerge } from "tailwind-merge"

import type { WorkingCopyEntry } from '../types/svn'

// ===== 工作副本智能标签推断（本地路径结构优先 + SVN 分支信息辅助） =====
// 用于左侧列表、面包屑等处自动区分大量同名模块（rest/front 等）和个人分支

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

export function getLocalProject(path: string): string | null {
  const parts = path.split(/[\\/]/).filter(Boolean)
  const workIdx = parts.findIndex((p) => p.toLowerCase() === 'work')
  if (workIdx >= 0 && workIdx + 1 < parts.length) {
    return parts[workIdx + 1]
  }
  if (parts.length >= 3) {
    const cand = parts[parts.length - 3]
    const lower = cand.toLowerCase()
    if (!['develop', 'produce', 'test', 'database', 'front', 'rest', 'updatesql'].includes(lower)) {
      return cand
    }
  }
  return null
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
  const parts = wc.path.split(/[\\/]/).filter(Boolean)
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
  const branchInfo = wc.relativeUrl || wc.url || ''
  return branchInfo ? `${loc}\n${branchInfo}` : loc
}
