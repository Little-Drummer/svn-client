<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import * as monaco from 'monaco-editor'

import EmptyState from '@/components/ui-local/EmptyState.vue'
import LoadingSpinner from '@/components/ui-local/LoadingSpinner.vue'
import { ensureMonacoEnv } from '../composables/monaco-setup'
import { currentSvnTheme, registerSvnThemes } from '../composables/monaco-theme'

const props = defineProps<{
  // svn diff 文本用于识别二进制文件。
  diffText: string | null
  // 左右对比需要原文件 BASE 内容和当前内容。
  baseContent?: string | null
  currentContent?: string | null
  filename?: string | null
  loading?: boolean
}>()

const containerRef = ref<HTMLDivElement | null>(null)
const diffInstance = shallowRef<monaco.editor.IStandaloneDiffEditor | null>(null)

// 系统主题变化时切换 monaco 主题，避免暗色界面里出现白底 diff
let colorSchemeQuery: MediaQueryList | null = null
function onColorSchemeChange() {
  monaco.editor.setTheme(currentSvnTheme())
}

const isBinary = computed(() => {
  // 二进制文件 svn diff 会输出 "Cannot display: file marked as a binary type."
  return /Cannot display:.*binary/i.test(props.diffText ?? '')
})

const hasSplitContent = computed(
  () => props.baseContent != null && props.currentContent != null,
)

const isEmpty = computed(() => {
  if (!hasSplitContent.value) return true
  return (props.baseContent ?? '') === '' && (props.currentContent ?? '') === ''
})

function disposeSplitModels() {
  const m = diffInstance.value?.getModel()
  m?.original.dispose()
  m?.modified.dispose()
}

function disposeAll() {
  disposeSplitModels()
  diffInstance.value?.dispose()
  diffInstance.value = null
}

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

function createSplit() {
  if (!containerRef.value) return
  const lang = detectLanguage(props.filename)
  diffInstance.value = monaco.editor.createDiffEditor(containerRef.value, {
    readOnly: true,
    theme: currentSvnTheme(),
    automaticLayout: true,
    renderSideBySide: true,
    minimap: { enabled: false },
    fontSize: 12,
    fontFamily: "'SF Mono', ui-monospace, Menlo, Consolas, monospace",
    fontLigatures: false,
    lineHeight: 18,
    padding: { top: 8, bottom: 8 },
    renderIndicators: true,
    renderOverviewRuler: true,
    smoothScrolling: true,
  })
  diffInstance.value.setModel({
    original: monaco.editor.createModel(props.baseContent ?? '', lang),
    modified: monaco.editor.createModel(props.currentContent ?? '', lang),
  })
}

// 固定使用左右对比，同一编辑器内只替换模型，避免文件切换时反复重建实例。
function refresh() {
  if (isBinary.value || isEmpty.value) {
    disposeAll()
    return
  }
  if (!containerRef.value) {
    // 模板还没渲染出宿主节点，等下一次 flush（watcher 用 post）
    return
  }
  if (!diffInstance.value) {
    createSplit()
    return
  }
  const lang = detectLanguage(props.filename)
  disposeSplitModels()
  diffInstance.value.setModel({
    original: monaco.editor.createModel(props.baseContent ?? '', lang),
    modified: monaco.editor.createModel(props.currentContent ?? '', lang),
  })
}

onMounted(() => {
  ensureMonacoEnv()
  registerSvnThemes()
  if (typeof matchMedia === 'function') {
    colorSchemeQuery = matchMedia('(prefers-color-scheme: dark)')
    colorSchemeQuery.addEventListener?.('change', onColorSchemeChange)
  }
  refresh()
})

onBeforeUnmount(() => {
  colorSchemeQuery?.removeEventListener?.('change', onColorSchemeChange)
  disposeAll()
})

// flush: 'post' 保证 isEmpty/isBinary 变化导致的 DOM 重渲染后再 refresh，containerRef 才是有效的
watch(
  () => [props.diffText, props.baseContent, props.currentContent, props.filename],
  () => refresh(),
  { flush: 'post' },
)
</script>

<template>
  <div class="diff-wrap">
    <div class="diff-body">
      <LoadingSpinner v-if="loading" />
      <div v-else-if="isBinary" class="hint">二进制文件，无法在编辑器中对比。</div>
      <EmptyState v-else-if="isEmpty" description="没有改动" />
      <div v-else ref="containerRef" class="monaco-host" />
    </div>
  </div>
</template>

<style scoped>
.diff-wrap {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--mat-content);
}
.diff-body {
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: stretch;
  justify-content: stretch;
}
.diff-body .monaco-host {
  flex: 1;
}
.hint {
  margin: 32px auto;
  color: var(--fg-muted);
  font-size: var(--fs-callout);
}
</style>
