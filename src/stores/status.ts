import { defineStore } from 'pinia'
import { ref } from 'vue'

import { api } from '../api/svn'
import type { SvnStatusEntry } from '../types/svn'

export const useStatusStore = defineStore('status', () => {
  const entries = ref<SvnStatusEntry[]>([])
  const loading = ref(false)
  const lastError = ref<string | null>(null)
  const showUnversioned = ref(true)

  async function reload(path: string | null | undefined) {
    if (!path) {
      entries.value = []
      return
    }
    loading.value = true
    lastError.value = null
    try {
      entries.value = await api.status(path, showUnversioned.value)
    } catch (e: unknown) {
      lastError.value = String((e as { message?: string })?.message ?? e)
      entries.value = []
    } finally {
      loading.value = false
    }
  }

  return { entries, loading, lastError, showUnversioned, reload }
})
