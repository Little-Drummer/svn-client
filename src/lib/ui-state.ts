// 侧栏焦点与工作副本展开状态的本地持久化。
// 焦点是「单一来源」：工作副本与远端仓库互斥，只记录上次真正聚焦的那一侧。

export type FocusSide = 'wc' | 'repo'

export interface FocusState {
  side: FocusSide
  id: string
}

const FOCUS_KEY = 'ui.lastFocus'
const WC_COLLAPSED_KEY = 'ui.wcCollapsedNodes'

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
