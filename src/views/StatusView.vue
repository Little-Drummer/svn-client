<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import {
  NButton,
  NCheckbox,
  NEmpty,
  NScrollbar,
  NSpin,
  NSwitch,
  NTag,
  useDialog,
} from 'naive-ui'

import DiffViewer from '../components/DiffViewer.vue'
import CommitPanel from '../components/CommitPanel.vue'
import UpdatePanel from '../components/UpdatePanel.vue'

import { api } from '../api/svn'
import { useStatusStore } from '../stores/status'
import { useErrorToast } from '../composables/use-error-toast'
import type { SvnStatusEntry, WorkingCopyEntry } from '../types/svn'

const props = defineProps<{ workingCopy: WorkingCopyEntry }>()

const statusStore = useStatusStore()
const toast = useErrorToast()
const dialog = useDialog()

const selectedFile = ref<SvnStatusEntry | null>(null)
const checkedPaths = ref<Set<string>>(new Set())
const diffText = ref<string | null>(null)
const baseContent = ref<string | null>(null)
const currentContent = ref<string | null>(null)
const diffLoading = ref(false)
const showUpdate = ref(false)

// 状态分组顺序
const STATUS_ORDER = [
  'conflicted',
  'modified',
  'added',
  'deleted',
  'replaced',
  'missing',
  'obstructed',
  'unversioned',
  'ignored',
  'incomplete',
  'external',
  'normal',
]
const STATUS_LABEL: Record<string, string> = {
  modified: '已修改',
  added: '新增',
  deleted: '删除',
  replaced: '替换',
  missing: '丢失',
  conflicted: '冲突',
  unversioned: '未跟踪',
  ignored: '忽略',
  obstructed: '阻塞',
  external: '外部',
  incomplete: '未完成',
  normal: '正常',
}
const STATUS_COLORS: Record<string, 'default' | 'success' | 'info' | 'warning' | 'error'> = {
  modified: 'info',
  added: 'success',
  deleted: 'error',
  replaced: 'warning',
  conflicted: 'error',
  missing: 'warning',
  obstructed: 'warning',
  unversioned: 'default',
  ignored: 'default',
  external: 'default',
  incomplete: 'warning',
  normal: 'default',
}

const grouped = computed(() => {
  const map = new Map<string, SvnStatusEntry[]>()
  for (const e of statusStore.entries) {
    const k = e.item || 'normal'
    if (!map.has(k)) map.set(k, [])
    map.get(k)!.push(e)
  }
  return STATUS_ORDER.filter((s) => map.has(s)).map((s) => ({
    item: s,
    label: STATUS_LABEL[s] ?? s,
    color: STATUS_COLORS[s] ?? 'default',
    entries: map.get(s)!.sort((a, b) => a.path.localeCompare(b.path)),
  }))
})

// 可提交的项（不能 commit unversioned/ignored/normal/missing/external）
const COMMITTABLE = new Set([
  'modified',
  'added',
  'deleted',
  'replaced',
  'conflicted',
])

const allCommittable = computed(() =>
  statusStore.entries.filter((e) => COMMITTABLE.has(e.item)),
)
const allChecked = computed(
  () =>
    allCommittable.value.length > 0 &&
    allCommittable.value.every((e) => checkedPaths.value.has(e.path)),
)

function toggleAll(v: boolean) {
  if (v) {
    for (const e of allCommittable.value) checkedPaths.value.add(e.path)
  } else {
    for (const e of allCommittable.value) checkedPaths.value.delete(e.path)
  }
}

function toggleEntry(e: SvnStatusEntry, v: boolean) {
  if (v) checkedPaths.value.add(e.path)
  else checkedPaths.value.delete(e.path)
}

async function reload() {
  await statusStore.reload(props.workingCopy.path)
  // 清掉已经不存在的勾选
  const exists = new Set(statusStore.entries.map((e) => e.path))
  for (const p of [...checkedPaths.value]) {
    if (!exists.has(p)) checkedPaths.value.delete(p)
  }
  // 选中状态保留：找回当前选中文件
  if (selectedFile.value) {
    const found = statusStore.entries.find((e) => e.path === selectedFile.value!.path)
    selectedFile.value = found ?? null
  }
}

watch(
  () => props.workingCopy.id,
  () => {
    selectedFile.value = null
    checkedPaths.value = new Set()
    reload().catch(toast)
  },
  { immediate: false },
)

onMounted(() => {
  reload().catch(toast)
})

watch(selectedFile, async (entry) => {
  diffText.value = null
  baseContent.value = null
  currentContent.value = null
  if (!entry) return
  // unversioned/ignored 不能 svn diff
  if (entry.item === 'unversioned' || entry.item === 'ignored' || entry.item === 'missing') {
    diffText.value = ''
    return
  }
  diffLoading.value = true
  try {
    diffText.value = await api.diff(entry.path)
    // 尝试加载左右对比所需的两份内容
    try {
      baseContent.value = await api.baseContent(entry.path)
    } catch {
      baseContent.value = ''
    }
    try {
      currentContent.value = await api.readFileText(entry.path)
    } catch {
      currentContent.value = ''
    }
  } catch (e) {
    toast(e, '加载 diff 失败')
  } finally {
    diffLoading.value = false
  }
})

function shortName(p: string) {
  // 相对工作副本根目录的相对显示，缩短面包屑长度
  const root = props.workingCopy.path
  if (p.startsWith(root)) {
    const rel = p.slice(root.length).replace(/^[\\/]+/, '')
    return rel || p
  }
  return p
}

async function revertSelected() {
  if (checkedPaths.value.size === 0) return
  const paths = [...checkedPaths.value]
  dialog.warning({
    title: '撤销本地修改',
    content: `这些文件将恢复到 BASE 版本，本地未提交的修改会丢失：\n${paths.join('\n')}`,
    positiveText: '确认撤销',
    negativeText: '取消',
    onPositiveClick: async () => {
      try {
        await api.revert(paths)
        await reload()
      } catch (e) {
        toast(e, '撤销失败')
      }
    },
  })
}
</script>

<template>
  <div class="status-view">
    <!-- 左：文件列表 -->
    <section class="file-list">
      <div class="list-toolbar">
        <n-checkbox
          :checked="allChecked"
          :indeterminate="!allChecked && [...checkedPaths].length > 0"
          @update:checked="(v: boolean) => toggleAll(v)"
        >
          全选可提交项
        </n-checkbox>
        <span class="spacer" />
        <n-switch
          v-model:value="statusStore.showUnversioned"
          size="small"
          @update:value="reload"
        />
        <span class="hint">显示未跟踪</span>
        <n-button size="tiny" tertiary @click="reload">刷新</n-button>
      </div>
      <n-scrollbar class="list-scroll">
        <n-spin v-if="statusStore.loading" />
        <n-empty
          v-else-if="grouped.length === 0"
          description="工作区干净，没有变更"
          size="small"
        />
        <div v-for="group in grouped" :key="group.item" class="group">
          <div class="group-header">
            <n-tag size="small" :type="group.color">{{ group.label }}</n-tag>
            <span class="group-count mono">{{ group.entries.length }}</span>
          </div>
          <div
            v-for="e in group.entries"
            :key="e.path"
            :class="['file-row', { active: selectedFile?.path === e.path }]"
            @click="selectedFile = e"
          >
            <n-checkbox
              :disabled="!COMMITTABLE.has(e.item)"
              :checked="checkedPaths.has(e.path)"
              @update:checked="(v: boolean) => toggleEntry(e, v)"
              @click.stop
            />
            <span class="file-path mono" :title="e.path">{{ shortName(e.path) }}</span>
          </div>
        </div>
      </n-scrollbar>
    </section>

    <!-- 中：diff -->
    <section class="diff-pane">
      <DiffViewer
        :diff-text="diffText"
        :base-content="baseContent"
        :current-content="currentContent"
        :filename="selectedFile?.path"
        :loading="diffLoading"
      />
    </section>

    <!-- 右：commit/update 面板 -->
    <section class="side-pane">
      <div class="side-tabs">
        <n-button
          size="small"
          :type="!showUpdate ? 'primary' : 'tertiary'"
          @click="showUpdate = false"
        >
          提交
        </n-button>
        <n-button
          size="small"
          :type="showUpdate ? 'primary' : 'tertiary'"
          @click="showUpdate = true"
        >
          更新
        </n-button>
        <span class="spacer" />
        <n-button
          size="small"
          tertiary
          type="warning"
          :disabled="checkedPaths.size === 0"
          @click="revertSelected"
        >
          撤销
        </n-button>
      </div>
      <CommitPanel
        v-if="!showUpdate"
        :working-copy="workingCopy"
        :checked-paths="[...checkedPaths]"
        @done="reload"
      />
      <UpdatePanel v-else :working-copy="workingCopy" @done="reload" />
    </section>
  </div>
</template>

<style scoped>
.status-view {
  display: grid;
  grid-template-columns: 320px 1fr 320px;
  height: 100%;
  min-height: 0;
}
.file-list,
.diff-pane,
.side-pane {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
}
.file-list {
  border-right: 1px solid rgba(127, 127, 127, 0.2);
}
.side-pane {
  border-left: 1px solid rgba(127, 127, 127, 0.2);
}
.list-toolbar {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  border-bottom: 1px solid rgba(127, 127, 127, 0.2);
  font-size: 12px;
}
.spacer {
  flex: 1;
}
.list-scroll {
  flex: 1;
  min-height: 0;
}
.group-header {
  display: flex;
  gap: 6px;
  align-items: center;
  padding: 4px 8px;
  background: rgba(127, 127, 127, 0.06);
}
.group-count {
  font-size: 11px;
  opacity: 0.6;
}
.file-row {
  display: flex;
  gap: 6px;
  align-items: center;
  padding: 3px 8px 3px 12px;
  cursor: pointer;
}
.file-row:hover {
  background: rgba(127, 127, 127, 0.08);
}
.file-row.active {
  background: rgba(26, 107, 255, 0.15);
}
.file-path {
  flex: 1;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.side-tabs {
  display: flex;
  gap: 6px;
  padding: 6px 8px;
  border-bottom: 1px solid rgba(127, 127, 127, 0.2);
  align-items: center;
}
.hint {
  font-size: 12px;
  opacity: 0.7;
}
</style>
