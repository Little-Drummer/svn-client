import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

import { api } from '../api/svn'
import type { WorkingCopyEntry } from '../types/svn'

export const useWorkingCopiesStore = defineStore('workingCopies', () => {
  const items = ref<WorkingCopyEntry[]>([])
  const selectedId = ref<string | null>(null)
  const loading = ref(false)

  const selected = computed(() => items.value.find((wc) => wc.id === selectedId.value) ?? null)

  async function reload() {
    loading.value = true
    try {
      items.value = await api.listWorkingCopies()
      if (!selectedId.value && items.value.length > 0) {
        selectedId.value = items.value[0].id
      }
      if (selectedId.value && !items.value.find((wc) => wc.id === selectedId.value)) {
        selectedId.value = items.value[0]?.id ?? null
      }
    } finally {
      loading.value = false
    }
  }

  async function add(path: string) {
    const entry = await api.addWorkingCopy(path)
    const idx = items.value.findIndex((wc) => wc.id === entry.id)
    if (idx >= 0) {
      items.value[idx] = entry
    } else {
      items.value.push(entry)
    }
    selectedId.value = entry.id
    return entry
  }

  async function remove(id: string) {
    await api.removeWorkingCopy(id)
    items.value = items.value.filter((wc) => wc.id !== id)
    if (selectedId.value === id) {
      selectedId.value = items.value[0]?.id ?? null
    }
  }

  async function refresh(id: string) {
    const entry = await api.refreshWorkingCopy(id)
    const idx = items.value.findIndex((wc) => wc.id === id)
    if (idx >= 0) items.value[idx] = entry
    return entry
  }

  function select(id: string | null) {
    selectedId.value = id
  }

  return { items, selectedId, selected, loading, reload, add, remove, refresh, select }
})
