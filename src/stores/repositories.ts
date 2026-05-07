import { defineStore } from 'pinia'
import { ref } from 'vue'

import { api } from '../api/svn'
import type { RepositoryEntry } from '../types/svn'

export const useRepositoriesStore = defineStore('repositories', () => {
  const items = ref<RepositoryEntry[]>([])
  const loading = ref(false)

  async function reload() {
    loading.value = true
    try {
      items.value = await api.listRepositories()
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
  }

  return { items, loading, reload, save, remove }
})
