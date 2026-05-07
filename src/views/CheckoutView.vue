<script setup lang="ts">
import { open } from '@tauri-apps/plugin-dialog'
import { computed, ref, watch } from 'vue'
import {
  NButton,
  NCard,
  NForm,
  NFormItem,
  NInput,
  NInputGroup,
  NSpace,
  useMessage,
} from 'naive-ui'

import TaskOutput from '../components/TaskOutput.vue'
import { api, describeError } from '../api/svn'
import { useTasksStore } from '../stores/tasks'
import { useWorkingCopiesStore } from '../stores/workingCopies'
import type { RepositoryEntry } from '../types/svn'

const props = defineProps<{ repository?: RepositoryEntry | null }>()

const tasksStore = useTasksStore()
const wcStore = useWorkingCopiesStore()
const message = useMessage()

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
    message.warning('URL 和目标目录都不能为空')
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
    message.error(describeError(e))
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
        message.success('检出完成，已加入工作副本列表')
      } catch (e) {
        message.warning('检出完成但加入工作副本列表失败：' + describeError(e))
      }
    }
  },
)
</script>

<template>
  <div class="checkout-view">
    <n-card title="检出 (svn checkout)" size="small" class="card">
      <n-form label-placement="left" label-width="100" size="small">
        <n-form-item label="远端 URL" required>
          <n-input
            v-model:value="url"
            placeholder="https://example.com/svn/repo/trunk"
            :disabled="running"
          />
        </n-form-item>
        <n-form-item label="本地目录" required>
          <n-input-group>
            <n-input
              v-model:value="targetPath"
              placeholder="/path/to/local/folder"
              :disabled="running"
            />
            <n-button :disabled="running" @click="pickTarget">选择…</n-button>
          </n-input-group>
        </n-form-item>
        <n-form-item label="Revision">
          <n-input v-model:value="revision" placeholder="留空 = HEAD" :disabled="running" />
        </n-form-item>
        <n-form-item label="用户名">
          <n-input v-model:value="username" placeholder="可选" :disabled="running" />
        </n-form-item>
        <n-form-item label="密码">
          <n-input
            v-model:value="password"
            type="password"
            show-password-on="click"
            placeholder="可选"
            :disabled="running"
          />
        </n-form-item>
        <n-space justify="end">
          <n-button type="primary" :loading="running" @click="start">开始检出</n-button>
        </n-space>
      </n-form>
    </n-card>

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
  padding: 16px;
  gap: 14px;
  background: var(--panel-bg-muted);
}
.card {
  flex-shrink: 0;
  border: 1px solid var(--border);
  border-radius: 8px;
  box-shadow: var(--shadow-sm);
}
.output-wrap {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--panel-bg);
}
</style>
