<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import * as monaco from 'monaco-editor'

import EmptyState from '@/components/ui-local/EmptyState.vue'
import LoadingSpinner from '@/components/ui-local/LoadingSpinner.vue'
import { ensureMonacoEnv } from '../composables/monaco-setup'
import { currentSvnTheme, registerSvnThemes } from '../composables/monaco-theme'

type DiffMode = 'unified' | 'split'

const props = defineProps<{
  // unified diff 文本（来自 svn diff），用作 unified 模式直接展示
  diffText: string | null
  // split 模式需要原文件 BASE 内容 + 当前内容
  baseContent?: string | null
  currentContent?: string | null
  filename?: string | null
  loading?: boolean
  // 初始呈现模式；log 视图查看单文件改动时默认 split（左右对比）
  initialMode?: DiffMode
}>()

const mode = ref<DiffMode>(props.initialMode ?? 'unified')
const containerRef = ref<HTMLDivElement | null>(null)
const editorInstance = shallowRef<monaco.editor.IStandaloneCodeEditor | null>(null)
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

const isEmpty = computed(() => {
  // split 模式直接喂全文（diffText 可能为空），以两侧内容判断是否有可展示的东西
  if (mode.value === 'split') {
    return (props.baseContent ?? '') === '' && (props.currentContent ?? '') === ''
  }
  return !props.diffText || props.diffText.trim().length === 0
})

// 跟踪当前编辑器实际处于哪种模式；用于判断本轮 refresh 是"切模式"还是"换内容"
let activeMode: DiffMode | null = null

function disposeSplitModels() {
  const m = diffInstance.value?.getModel()
  m?.original.dispose()
  m?.modified.dispose()
}

function disposeAll() {
  disposeSplitModels()
  editorInstance.value?.dispose()
  diffInstance.value?.dispose()
  editorInstance.value = null
  diffInstance.value = null
  activeMode = null
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

function createUnified() {
  if (!containerRef.value) return
  editorInstance.value = monaco.editor.create(containerRef.value, {
    value: props.diffText ?? '',
    language: 'diff',
    readOnly: true,
    theme: currentSvnTheme(),
    automaticLayout: true,
    minimap: { enabled: false },
    renderWhitespace: 'selection',
    scrollBeyondLastLine: false,
    fontSize: 12,
    fontFamily: "'SF Mono', ui-monospace, Menlo, Consolas, monospace",
    fontLigatures: false,
    lineHeight: 18,
    padding: { top: 8, bottom: 8 },
    smoothScrolling: true,
    cursorBlinking: 'smooth',
    cursorSmoothCaretAnimation: 'on',
    renderLineHighlight: 'line',
    guides: { indentation: false },
  })
  activeMode = 'unified'
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
  activeMode = 'split'
}

// 同模式下只更新内容，避免重建编辑器；切模式才销毁重建
function refresh() {
  if (isBinary.value || isEmpty.value) {
    disposeAll()
    return
  }
  if (!containerRef.value) {
    // 模板还没渲染出宿主节点，等下一次 flush（watcher 用 post）
    return
  }
  if (activeMode !== mode.value) {
    disposeAll()
    if (mode.value === 'unified') createUnified()
    else createSplit()
    return
  }
  if (mode.value === 'unified' && editorInstance.value) {
    const next = props.diffText ?? ''
    if (editorInstance.value.getValue() !== next) {
      editorInstance.value.setValue(next)
    }
  } else if (mode.value === 'split' && diffInstance.value) {
    const lang = detectLanguage(props.filename)
    disposeSplitModels()
    diffInstance.value.setModel({
      original: monaco.editor.createModel(props.baseContent ?? '', lang),
      modified: monaco.editor.createModel(props.currentContent ?? '', lang),
    })
  }
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
  () => [props.diffText, props.baseContent, props.currentContent, props.filename, mode.value],
  () => refresh(),
  { flush: 'post' },
)
</script>

<template>
  <div class="diff-wrap">
    <div class="diff-toolbar">
      <div class="segmented">
        <button
          type="button"
          :class="['seg-btn', mode === 'unified' && 'is-active']"
          @click="mode = 'unified'"
        >
          Unified
        </button>
        <button
          type="button"
          :class="['seg-btn', mode === 'split' && 'is-active']"
          @click="mode = 'split'"
        >
          左右对比
        </button>
      </div>
      <span v-if="filename" class="diff-filename mono" :title="filename">{{ filename }}</span>
    </div>
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
.diff-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  height: 36px;
  flex: none;
  padding: 0 12px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
  background: var(--mat-toolbar);
  backdrop-filter: var(--vibrancy-toolbar);
  -webkit-backdrop-filter: var(--vibrancy-toolbar);
}
.segmented {
  display: inline-flex;
  height: 22px;
  padding: 2px;
  gap: 2px;
  border-radius: 7px;
  background: rgba(0, 0, 0, 0.06);
  border: var(--hairline) solid var(--stroke-soft);
}
.dark .segmented {
  background: rgba(255, 255, 255, 0.05);
}
.seg-btn {
  height: 18px;
  padding: 0 8px;
  border: 0;
  background: transparent;
  border-radius: 5px;
  font-size: var(--fs-caption);
  font-weight: 500;
  color: var(--fg-muted);
  cursor: default;
  transition: background-color 140ms ease-out, color 140ms ease-out, box-shadow 160ms ease-out;
}
.seg-btn:hover {
  color: var(--fg);
}
.seg-btn.is-active {
  color: var(--fg-strong);
  background: var(--mat-elevated);
  box-shadow:
    inset 0 0 0 0.5px var(--stroke),
    0 1px 1.5px rgba(0, 0, 0, 0.06);
}
.dark .seg-btn.is-active {
  box-shadow:
    inset 0 0 0 0.5px rgba(255, 255, 255, 0.08),
    0 1px 1.5px rgba(0, 0, 0, 0.4);
}
.diff-filename {
  font-size: var(--fs-callout);
  color: var(--fg-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  flex: 1;
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
