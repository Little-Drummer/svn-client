// 侧栏焦点与工作副本展开状态的本地持久化。
// 焦点是「单一来源」：工作副本与远端仓库互斥，只记录上次真正聚焦的那一侧。

export type FocusSide = 'wc' | 'repo'

export interface FocusState {
  side: FocusSide
  id: string
}

const FOCUS_KEY = 'ui.lastFocus'
const WC_COLLAPSED_KEY = 'ui.wcCollapsedNodes'
const COMMIT_HISTORY_KEY = 'ui.commitHistory'
const MAX_COMMIT_HISTORY = 15

export function loadFocus(): FocusState | null {
  try {
    const raw = localStorage.getItem(FOCUS_KEY)
    if (!raw) return null
    const parsed = JSON.parse(raw)
    if (
      parsed &&
      (parsed.side === 'wc' || parsed.side === 'repo') &&
      typeof parsed.id === 'string'
    ) {
      return parsed
    }
  } catch {
    // 解析失败按无记录处理，不阻塞启动
  }
  return null
}

export function saveFocus(focus: FocusState): void {
  try {
    localStorage.setItem(FOCUS_KEY, JSON.stringify(focus))
  } catch {
    // 持久化失败不影响交互
  }
}

export function loadCollapsedNodes(): string[] {
  try {
    const raw = localStorage.getItem(WC_COLLAPSED_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    if (Array.isArray(parsed)) {
      return parsed.filter((k): k is string => typeof k === 'string')
    }
  } catch {
    // 同上
  }
  return []
}

export function saveCollapsedNodes(keys: string[]): void {
  try {
    localStorage.setItem(WC_COLLAPSED_KEY, JSON.stringify(keys))
  } catch {
    // 同上
  }
}

export function loadCommitHistory(): string[] {
  try {
    const raw = localStorage.getItem(COMMIT_HISTORY_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    if (Array.isArray(parsed)) {
      return parsed.filter((item): item is string => typeof item === 'string' && item.trim().length > 0)
    }
  } catch {
    // 解析失败按空历史处理
  }
  return []
}

// 记录最近提交说明：去重并置于最前，最多保留 MAX_COMMIT_HISTORY 条
export function pushCommitHistory(message: string): string[] {
  const trimmed = message.trim()
  if (!trimmed) return loadCommitHistory()
  const next = [trimmed, ...loadCommitHistory().filter((item) => item !== trimmed)].slice(
    0,
    MAX_COMMIT_HISTORY,
  )
  try {
    localStorage.setItem(COMMIT_HISTORY_KEY, JSON.stringify(next))
  } catch {
    // 持久化失败不影响提交流程
  }
  return next
}
