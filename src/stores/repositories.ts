import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import { api } from '../api/svn'
import { loadFocus, saveFocus } from '../lib/ui-state'
import type { RepositoryEntry } from '../types/svn'

export const useRepositoriesStore = defineStore('repositories', () => {
  const items = ref<RepositoryEntry[]>([])
  // 上次焦点在远端时恢复其选中
  const initialFocus = loadFocus()
  const selectedId = ref<string | null>(
    initialFocus?.side === 'repo' ? initialFocus.id : null,
  )
  const loading = ref(false)

  const selected = computed(() => items.value.find((repo) => repo.id === selectedId.value) ?? null)

  async function reload() {
    loading.value = true
    try {
      items.value = await api.listRepositories()
      // 持久化的选中项已不存在时清掉，避免高亮幽灵行
      if (selectedId.value && !items.value.some((repo) => repo.id === selectedId.value)) {
        selectedId.value = null
      }
    } finally {
      loading.value = false
    }
  }

  async function save(input: {
    id?: string
    name: string
    url: string
    username?: string
  }) {
    const entry = await api.saveRepository(input)
    const idx = items.value.findIndex((repo) => repo.id === entry.id)
    if (idx >= 0) items.value[idx] = entry
    else items.value.unshift(entry)
    return entry
  }

  async function remove(id: string) {
    await api.removeRepository(id)
    items.value = items.value.filter((repo) => repo.id !== id)
    if (selectedId.value === id) selectedId.value = null
  }

  function select(id: string | null) {
    selectedId.value = id
    if (id) saveFocus({ side: 'repo', id })
  }

  return { items, selectedId, selected, loading, reload, save, remove, select }
})
