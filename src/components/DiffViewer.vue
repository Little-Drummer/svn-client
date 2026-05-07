<script setup lang="ts">
import { NEmpty, NRadioButton, NRadioGroup, NSpin } from 'naive-ui'
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import * as monaco from 'monaco-editor'

import { ensureMonacoEnv } from '../composables/monaco-setup'

type DiffMode = 'unified' | 'split'

const props = defineProps<{
  // unified diff 文本（来自 svn diff），用作 unified 模式直接展示
  diffText: string | null
  // split 模式需要原文件 BASE 内容 + 当前内容
  baseContent?: string | null
  currentContent?: string | null
  filename?: string | null
  loading?: boolean
}>()

const mode = ref<DiffMode>('unified')
const containerRef = ref<HTMLDivElement | null>(null)
const editorInstance = shallowRef<monaco.editor.IStandaloneCodeEditor | null>(null)
const diffInstance = shallowRef<monaco.editor.IStandaloneDiffEditor | null>(null)

const isBinary = computed(() => {
  // 二进制文件 svn diff 会输出 "Cannot display: file marked as a binary type."
  return /Cannot display:.*binary/i.test(props.diffText ?? '')
})

const isEmpty = computed(() => !props.diffText || props.diffText.trim().length === 0)

function disposeAll() {
  editorInstance.value?.dispose()
  diffInstance.value?.dispose()
  editorInstance.value = null
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

function ensureUnified() {
  if (!containerRef.value) return
  disposeAll()
  editorInstance.value = monaco.editor.create(containerRef.value, {
    value: props.diffText ?? '',
    language: 'diff',
    readOnly: true,
    theme: matchMedia('(prefers-color-scheme: dark)').matches ? 'vs-dark' : 'vs',
    automaticLayout: true,
    minimap: { enabled: false },
    renderWhitespace: 'selection',
    scrollBeyondLastLine: false,
    fontSize: 12,
  })
}

function ensureSplit() {
  if (!containerRef.value) return
  disposeAll()
  const lang = detectLanguage(props.filename)
  diffInstance.value = monaco.editor.createDiffEditor(containerRef.value, {
    readOnly: true,
    theme: matchMedia('(prefers-color-scheme: dark)').matches ? 'vs-dark' : 'vs',
    automaticLayout: true,
    renderSideBySide: true,
    minimap: { enabled: false },
    fontSize: 12,
  })
  diffInstance.value.setModel({
    original: monaco.editor.createModel(props.baseContent ?? '', lang),
    modified: monaco.editor.createModel(props.currentContent ?? '', lang),
  })
}

function refresh() {
  if (isBinary.value || isEmpty.value) {
    disposeAll()
    return
  }
  if (mode.value === 'unified') {
    ensureUnified()
  } else {
    ensureSplit()
  }
}

onMounted(() => {
  ensureMonacoEnv()
  refresh()
})

onBeforeUnmount(() => {
  disposeAll()
})

watch(
  () => [props.diffText, props.baseContent, props.currentContent, props.filename, mode.value],
  () => refresh(),
)
</script>

<template>
  <div class="diff-wrap">
    <div class="diff-toolbar">
      <n-radio-group v-model:value="mode" size="small">
        <n-radio-button value="unified">Unified</n-radio-button>
        <n-radio-button value="split">左右对比</n-radio-button>
      </n-radio-group>
      <span v-if="filename" class="diff-filename mono">{{ filename }}</span>
    </div>
    <div class="diff-body">
      <n-spin v-if="loading" />
      <div v-else-if="isBinary" class="hint">二进制文件，无法在编辑器中对比。</div>
      <n-empty v-else-if="isEmpty" description="没有改动" size="small" />
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
}
.diff-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 10px;
  border-bottom: 1px solid rgba(127, 127, 127, 0.2);
}
.diff-filename {
  font-size: 12px;
  opacity: 0.75;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
  margin: 24px;
  opacity: 0.7;
}
</style>
