import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

import { api } from '../api/svn'
import { loadFocus, saveFocus } from '../lib/ui-state'
import type { WorkingCopyEntry } from '../types/svn'

export const useWorkingCopiesStore = defineStore('workingCopies', () => {
  const items = ref<WorkingCopyEntry[]>([])
  // 上次焦点在工作副本时先恢复其 id，reload 拿到列表后再校验
  const initialFocus = loadFocus()
  const selectedId = ref<string | null>(
    initialFocus?.side === 'wc' ? initialFocus.id : null,
  )
  const loading = ref(false)
  // 只在会影响“项目 → 分支 → 模块”聚合结果时递增，供常驻页面同步刷新项目数据。
  const projectTopologyVersion = ref(0)
  // 携带具体工作副本 ID，通知侧栏只重新读取对应项目的状态数量。
  const statusRefreshRequest = ref<{ id: string; version: number } | null>(null)
  let statusRefreshVersion = 0

  const selected = computed(() => items.value.find((wc) => wc.id === selectedId.value) ?? null)

  function exists(id: string | null) {
    return !!id && items.value.some((wc) => wc.id === id)
  }

  function topologySignature(entries: WorkingCopyEntry[]) {
    return entries
      .map((entry) =>
        [
          entry.id,
          entry.path,
          entry.url ?? '',
          entry.repositoryRoot ?? '',
          entry.relativeUrl ?? '',
        ].join('\u0000'),
      )
      .sort()
      .join('\n')
  }

  function replaceItems(next: WorkingCopyEntry[]) {
    const changed = topologySignature(items.value) !== topologySignature(next)
    items.value = next
    if (changed) projectTopologyVersion.value += 1
  }

  async function reload() {
    loading.value = true
    try {
      replaceItems(await api.listWorkingCopies())
      const focus = loadFocus()
      if (focus?.side === 'repo') {
        // 上次焦点在远端：工作副本不抢焦点，保持未选中
        selectedId.value = null
      } else if (focus?.side === 'wc' && exists(focus.id)) {
        selectedId.value = focus.id
      } else if (!exists(selectedId.value)) {
        // 无有效持久化选中时回退到第一个
        selectedId.value = items.value[0]?.id ?? null
      }
    } finally {
      loading.value = false
    }
  }

  async function add(path: string) {
    const entry = await api.addWorkingCopy(path)
    const idx = items.value.findIndex((wc) => wc.id === entry.id)
    const next = [...items.value]
    if (idx >= 0) {
      next[idx] = entry
    } else {
      next.push(entry)
    }
    replaceItems(next)
    selectedId.value = entry.id
    return entry
  }

  // 扫描项目根目录，批量识别并加入其下所有分支/模块工作副本，返回新增/更新的数量
  async function scanProject(path: string) {
    const touched = await api.scanAndAddProject(path)
    await reload()
    return touched
  }

  // 把当前工作副本聚合成 项目 → 环境(分支) → 模块 视图（供合并/打包使用）
  function listProjects() {
    return api.listProjects()
  }

  async function remove(id: string) {
    await api.removeWorkingCopy(id)
    replaceItems(items.value.filter((wc) => wc.id !== id))
    if (selectedId.value === id) {
      selectedId.value = items.value[0]?.id ?? null
    }
  }

  async function refresh(id: string) {
    const entry = await api.refreshWorkingCopy(id)
    const idx = items.value.findIndex((wc) => wc.id === id)
    if (idx >= 0) {
      const next = [...items.value]
      next[idx] = entry
      replaceItems(next)
    }
    statusRefreshRequest.value = { id, version: ++statusRefreshVersion }
    return entry
  }

  async function setDisplayName(id: string, displayName: string | null) {
    const entry = await api.setWorkingCopyDisplayName(id, displayName)
    const idx = items.value.findIndex((wc) => wc.id === id)
    if (idx >= 0) items.value[idx] = entry
    return entry
  }

  function select(id: string | null) {
    selectedId.value = id
    // 只有真正选中工作副本才记录焦点；置空交由对侧（远端）记录自己的焦点
    if (id) saveFocus({ side: 'wc', id })
  }

  return {
    items,
    selectedId,
    selected,
    loading,
    projectTopologyVersion,
    statusRefreshRequest,
    reload,
    add,
    scanProject,
    listProjects,
    remove,
    refresh,
    setDisplayName,
    select,
  }
})
