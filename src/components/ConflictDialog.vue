<script setup lang="ts">
import { onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import * as monaco from 'monaco-editor'

import { api } from '../api/svn'
import type { ConflictDetails } from '../types/svn'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import LoadingSpinner from '@/components/ui-local/LoadingSpinner.vue'
import { ensureMonacoEnv } from '../composables/monaco-setup'
import { currentSvnTheme, registerSvnThemes } from '../composables/monaco-theme'
import { useConfirmDialog } from '../composables/use-confirm-dialog'
import { useErrorToast } from '../composables/use-error-toast'
import { useAppToast } from '../composables/use-app-toast'

const props = defineProps<{ path: string | null; open: boolean }>()
const emit = defineEmits<{ 'update:open': [boolean]; resolved: [] }>()

const { confirm } = useConfirmDialog()
const toast = useErrorToast()
const appToast = useAppToast()

const details = ref<ConflictDetails | null>(null)
const loading = ref(false)
const working = ref(false)
// compare：mine/theirs 只读对比；edit：直接编辑带冲突标记的工作文件
const mode = ref<'compare' | 'edit'>('compare')

const containerRef = ref<HTMLDivElement | null>(null)
const diffInstance = shallowRef<monaco.editor.IStandaloneDiffEditor | null>(null)
const editInstance = shallowRef<monaco.editor.IStandaloneCodeEditor | null>(null)

function detectLanguage(name: string | null | undefined): string {
  if (!name) return 'plaintext'
  const ext = name.toLowerCase().split('.').pop() ?? ''
  const map: Record<string, string> = {
    ts: 'typescript', tsx: 'typescript', js: 'javascript', jsx: 'javascript',
    vue: 'html', html: 'html', css: 'css', scss: 'scss',
    json: 'json', md: 'markdown', xml: 'xml', yaml: 'yaml', yml: 'yaml',
    rs: 'rust', go: 'go', py: 'python', java: 'java', kt: 'kotlin',
    c: 'c', h: 'c', cpp: 'cpp', cc: 'cpp', hpp: 'cpp',
    sh: 'shell', sql: 'sql',
  }
  return map[ext] ?? 'plaintext'
}

function disposeEditors() {
  const m = diffInstance.value?.getModel()
  m?.original.dispose()
  m?.modified.dispose()
  diffInstance.value?.dispose()
  diffInstance.value = null
  editInstance.value?.getModel()?.dispose()
  editInstance.value?.dispose()
  editInstance.value = null
}

const EDITOR_OPTIONS = {
  theme: undefined as string | undefined,
  automaticLayout: true,
  minimap: { enabled: false },
  fontSize: 12,
  fontFamily: "'SF Mono', ui-monospace, Menlo, Consolas, monospace",
  fontLigatures: false,
  lineHeight: 18,
  padding: { top: 8, bottom: 8 },
  smoothScrolling: true,
}

function rebuildEditors() {
  disposeEditors()
  const host = containerRef.value
  const d = details.value
  if (!host || !d) return
  ensureMonacoEnv()
  registerSvnThemes()
  const lang = detectLanguage(d.path)
  if (mode.value === 'compare') {
    diffInstance.value = monaco.editor.createDiffEditor(host, {
      ...EDITOR_OPTIONS,
      theme: currentSvnTheme(),
      readOnly: true,
      renderSideBySide: true,
    })
    diffInstance.value.setModel({
      original: monaco.editor.createModel(d.mineContent ?? '', lang),
      modified: monaco.editor.createModel(d.theirsContent ?? '', lang),
    })
  } else {
    editInstance.value = monaco.editor.create(host, {
      ...EDITOR_OPTIONS,
      theme: currentSvnTheme(),
      value: d.workingContent,
      language: lang,
    })
  }
}

async function load() {
  if (!props.path) return
  loading.value = true
  details.value = null
  mode.value = 'compare'
  try {
    details.value = await api.conflictDetails(props.path)
  } catch (e) {
    toast(e, '读取冲突详情失败')
    emit('update:open', false)
  } finally {
    loading.value = false
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) void load()
    else disposeEditors()
  },
)

// details/mode 变化后等 DOM 渲染出宿主节点再重建编辑器
watch([details, mode], () => rebuildEditors(), { flush: 'post' })

onBeforeUnmount(() => disposeEditors())

function shortName(p: string) {
  return p.split('/').pop() ?? p
}

async function resolveWith(accept: 'mine-full' | 'theirs-full', label: string) {
  if (!props.path || working.value) return
  const ok = await confirm({
    title: `采用${label}`,
    content: `整个文件将使用${label}的内容，另一方的修改会丢弃：\n${props.path}`,
    confirmText: '确认',
    destructive: true,
  })
  if (!ok) return
  await doResolve(accept)
}

async function resolveEdited() {
  if (!props.path || working.value || !editInstance.value) return
  const content = editInstance.value.getValue()
  if (/^(<{7}|={7}|>{7})/m.test(content)) {
    const ok = await confirm({
      title: '内容仍包含冲突标记',
      content: '编辑结果里似乎还有 <<<<<<< / ======= / >>>>>>> 标记，确定按当前内容标记已解决？',
      confirmText: '仍然标记',
      destructive: true,
    })
    if (!ok) return
  }
  await doResolve('working', content)
}

async function resolveAsIs() {
  if (!props.path || working.value) return
  const ok = await confirm({
    title: '标记已解决',
    content: `按工作文件当前内容标记冲突已解决：\n${props.path}`,
    confirmText: '标记已解决',
  })
  if (!ok) return
  await doResolve('working')
}

async function doResolve(
  accept: 'working' | 'mine-full' | 'theirs-full',
  content?: string,
) {
  if (!props.path) return
  working.value = true
  try {
    await api.conflictResolve(props.path, accept, content)
    appToast.success('冲突已解决', shortName(props.path))
    emit('resolved')
    emit('update:open', false)
  } catch (e) {
    toast(e, '解决冲突失败')
  } finally {
    working.value = false
  }
}
</script>

<template>
  <Dialog :open="open" @update:open="(v) => emit('update:open', v)">
    <DialogContent class="conflict-modal">
      <DialogHeader>
        <DialogTitle>解决冲突 — {{ path ? shortName(path) : '' }}</DialogTitle>
      </DialogHeader>

      <LoadingSpinner v-if="loading" />
      <template v-else-if="details">
        <div class="conflict-toolbar">
          <div class="mode-switch">
            <Button
              size="xs"
              :variant="mode === 'compare' ? 'secondary' : 'ghost'"
              @click="mode = 'compare'"
            >
              对比双方
            </Button>
            <Button
              size="xs"
              :variant="mode === 'edit' ? 'secondary' : 'ghost'"
              @click="mode = 'edit'"
            >
              编辑结果
            </Button>
          </div>
          <span class="path mono" :title="details.path">{{ details.path }}</span>
        </div>

        <div v-if="mode === 'compare'" class="side-labels">
          <span>我的（本地修改）</span>
          <span>对方（更新 / 合并传入）</span>
        </div>
        <div v-else class="side-labels">
          <span>工作文件（含冲突标记，可直接编辑）</span>
        </div>

        <div ref="containerRef" class="monaco-host" />

        <div class="conflict-actions">
          <Button
            size="sm"
            variant="outline"
            :disabled="working"
            @click="resolveWith('mine-full', '我的版本')"
          >
            采用我的版本
          </Button>
          <Button
            size="sm"
            variant="outline"
            :disabled="working"
            @click="resolveWith('theirs-full', '对方版本')"
          >
            采用对方版本
          </Button>
          <span class="spacer" />
          <Button
            v-if="mode === 'edit'"
            size="sm"
            :disabled="working"
            @click="resolveEdited"
          >
            {{ working ? '处理中…' : '保存并标记已解决' }}
          </Button>
          <Button
            v-else
            size="sm"
            variant="ghost"
            :disabled="working"
            @click="resolveAsIs"
          >
            按当前内容标记已解决
          </Button>
        </div>
      </template>
    </DialogContent>
  </Dialog>
</template>

<style scoped>
.conflict-modal {
  width: min(1100px, 94vw);
  max-width: 94vw;
  display: flex;
  flex-direction: column;
}

.conflict-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.mode-switch {
  display: flex;
  gap: 2px;
  flex: none;
}

.path {
  font-size: 11px;
  color: var(--muted-foreground);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.side-labels {
  display: flex;
  margin-top: 8px;
  font-size: 11px;
  color: var(--muted-foreground);
}

.side-labels span {
  flex: 1;
}

.monaco-host {
  margin-top: 4px;
  height: 56vh;
  border: 1px solid var(--border);
  border-radius: 8px;
  overflow: hidden;
}

.conflict-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
}

.spacer {
  flex: 1;
}
</style>
