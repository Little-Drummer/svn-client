import { defineStore } from 'pinia'
import { ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import { api } from '../api/svn'
import { createGeneration } from '../composables/use-request-generation'
import type { SvnStatusEntry, StatusStreamEvent } from '../types/svn'

const STATUS_EVENT_NAME = 'svn-status-stream'

export const useStatusStore = defineStore('status', () => {
  const entries = ref<SvnStatusEntry[]>([])
  const loading = ref(false)
  const lastError = ref<string | null>(null)
  const showUnversioned = ref(true)

  // 代际令牌：切 WC 或快速重复刷新时，丢弃已被超越的请求结果，避免旧数据覆盖新数据
  const gen = createGeneration()

  // 当前生效的流式刷新 request_id，事件按它过滤，旧请求的批次直接丢弃
  let activeRequestId: string | null = null
  let unlisten: UnlistenFn | null = null

  // 懒注册一次全局事件监听，store 生命周期与应用一致，无需反复 listen/unlisten
  async function ensureListener() {
    if (unlisten) {
      return
    }
    unlisten = await listen<StatusStreamEvent>(STATUS_EVENT_NAME, (event) => {
      const payload = event.payload
      if (payload.requestId !== activeRequestId) {
        return
      }
      switch (payload.kind) {
        case 'entries':
          entries.value.push(...payload.entries)
          break
        case 'finished':
          loading.value = false
          break
        case 'failed':
          lastError.value = payload.message
          loading.value = false
          break
      }
    })
  }

  // 流式刷新：entry 分批到达即追加渲染，配合虚拟列表降低大工作副本首屏延迟
  async function reloadStreaming(path: string | null | undefined) {
    gen.invalidate()
    activeRequestId = null
    entries.value = []
    if (!path) {
      loading.value = false
      return
    }
    loading.value = true
    lastError.value = null
    try {
      await ensureListener()
      const requestId = await api.statusStream(path, showUnversioned.value)
      activeRequestId = requestId
    } catch (e: unknown) {
      lastError.value = String((e as { message?: string })?.message ?? e)
      entries.value = []
      loading.value = false
    }
  }

  // 一次性刷新（回退路径，用于不需要流式的小工作副本或调试）
  async function reload(path: string | null | undefined) {
    if (!path) {
      gen.invalidate()
      entries.value = []
      return
    }
    const token = gen.next()
    loading.value = true
    lastError.value = null
    try {
      const result = await api.status(path, showUnversioned.value)
      if (!gen.isCurrent(token)) return
      entries.value = result
    } catch (e: unknown) {
      if (!gen.isCurrent(token)) return
      lastError.value = String((e as { message?: string })?.message ?? e)
      entries.value = []
    } finally {
      if (gen.isCurrent(token)) loading.value = false
    }
  }

  return { entries, loading, lastError, showUnversioned, reload, reloadStreaming }
})
