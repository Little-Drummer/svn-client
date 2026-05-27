<script setup lang="ts">
import { open } from '@tauri-apps/plugin-dialog'
import { computed, ref, watch } from 'vue'

import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useAppToast } from '@/composables/use-app-toast'
import TaskOutput from '../components/TaskOutput.vue'
import { api, describeError } from '../api/svn'
import { useTasksStore } from '../stores/tasks'
import { useWorkingCopiesStore } from '../stores/workingCopies'
import type { RepositoryEntry } from '../types/svn'

const props = defineProps<{ repository?: RepositoryEntry | null }>()

const tasksStore = useTasksStore()
const wcStore = useWorkingCopiesStore()
const toast = useAppToast()

const url = ref('')
const targetPath = ref('')
const revision = ref('')
const username = ref('')
const password = ref('')

const taskId = ref<string | null>(null)
const running = computed(() => {
  if (!taskId.value) return false
  const t = tasksStore.tasks.get(taskId.value)
  return !!t && !t.finished
})

async function pickTarget() {
  const dir = await open({ directory: true, multiple: false, title: '选择检出目录' })
  if (typeof dir === 'string') {
    targetPath.value = dir
  }
}

async function start() {
  if (!url.value.trim() || !targetPath.value.trim()) {
    toast.warning('URL 和目标目录都不能为空')
    return
  }
  try {
    const id = await api.startCheckout({
      url: url.value.trim(),
      targetPath: targetPath.value.trim(),
      revision: revision.value || undefined,
      username: username.value || undefined,
      password: password.value || undefined,
    })
    tasksStore.register({
      taskId: id,
      kind: 'checkout',
      title: `检出 ${url.value} → ${targetPath.value}`,
    })
    taskId.value = id
  } catch (e) {
    toast.error('启动检出失败', describeError(e))
  }
}

watch(
  () => props.repository?.id,
  () => {
    if (!props.repository) return
    url.value = props.repository.url
    username.value = props.repository.username ?? ''
  },
  { immediate: true },
)

watch(
  () => taskId.value && tasksStore.tasks.get(taskId.value)?.finished,
  async (finished) => {
    if (!finished) return
    const t = taskId.value ? tasksStore.tasks.get(taskId.value) : null
    if (t?.success) {
      // 自动加入工作副本列表
      try {
        await wcStore.add(targetPath.value)
        toast.success('检出完成，已加入工作副本列表')
      } catch (e) {
        toast.warning('检出完成但加入工作副本列表失败', describeError(e))
      }
    }
  },
)
</script>

<template>
  <div class="checkout-view">
    <Card class="card">
      <CardHeader class="card-head">
        <CardTitle>检出 (svn checkout)</CardTitle>
      </CardHeader>
      <CardContent>
      <div class="checkout-form">
        <div class="form-row">
          <Label for="checkout-url">远端 URL</Label>
          <Input
            id="checkout-url"
            v-model="url"
            placeholder="https://example.com/svn/repo/trunk"
            :disabled="running"
          />
        </div>
        <div class="form-row">
          <Label for="checkout-target">本地目录</Label>
          <div class="input-group">
            <Input
              id="checkout-target"
              v-model="targetPath"
              placeholder="/path/to/local/folder"
              :disabled="running"
            />
            <Button variant="outline" :disabled="running" @click="pickTarget">选择…</Button>
          </div>
        </div>
        <div class="form-row">
          <Label for="checkout-revision">Revision</Label>
          <Input id="checkout-revision" v-model="revision" placeholder="留空 = HEAD" :disabled="running" />
        </div>
        <div class="form-row">
          <Label for="checkout-username">用户名</Label>
          <Input id="checkout-username" v-model="username" placeholder="可选" :disabled="running" />
        </div>
        <div class="form-row">
          <Label for="checkout-password">密码</Label>
          <Input
            id="checkout-password"
            v-model="password"
            type="password"
            placeholder="可选"
            :disabled="running"
          />
        </div>
        <div class="form-actions">
          <Button :disabled="running" @click="start">{{ running ? '检出中' : '开始检出' }}</Button>
        </div>
      </div>
      </CardContent>
    </Card>

    <div class="output-wrap">
      <TaskOutput :task-id="taskId" />
    </div>
  </div>
</template>

<style scoped>
.checkout-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  padding: 12px;
  gap: 12px;
  background: var(--panel-bg-muted);
}
.card {
  flex-shrink: 0;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--panel-bg);
  box-shadow: none;
}
.card-head {
  padding: 14px 16px 8px;
}
.checkout-form {
  display: grid;
  gap: 12px;
}
.form-row {
  display: grid;
  grid-template-columns: 100px minmax(0, 1fr);
  gap: 12px;
  align-items: center;
}
.form-row label {
  color: var(--text-muted);
  font-size: 12px;
}
.input-group {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
}
.form-actions {
  display: flex;
  justify-content: flex-end;
}
.output-wrap {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--panel-bg);
}
</style>
