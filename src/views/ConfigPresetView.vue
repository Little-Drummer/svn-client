<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { FilePlus2, Trash2, Wand2, FileText, X, ListOrdered, ChevronDown, ChevronRight } from 'lucide-vue-next'

import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import { api } from '../api/svn'
import { useWorkingCopiesStore } from '../stores/workingCopies'
import { useErrorToast } from '../composables/use-error-toast'
import { useAppToast } from '../composables/use-app-toast'
import { confirm } from '../composables/use-confirm-dialog'
import type { ConfigPreset, PresetApplyPlan, Project } from '../types/svn'

const props = defineProps<{ activeProjectName?: string | null }>()

const toast = useErrorToast()
const appToast = useAppToast()
const wcStore = useWorkingCopiesStore()

const projects = ref<Project[]>([])
const presets = ref<ConfigPreset[]>([])

// 左侧增删/扫描工作副本后，项目结构会变，实时刷新下拉选项（本视图常驻挂载）
watch(
  () => wcStore.items.length,
  async () => {
    try {
      projects.value = await api.listProjects()
    } catch (e) {
      toast(e, '刷新项目列表失败')
    }
  },
)

// 所有项目的模块拍平为应用目标，按项目分组展示
interface ModuleTarget {
  project: string
  label: string
  path: string
}
const moduleTargets = computed<ModuleTarget[]>(() => {
  const out: ModuleTarget[] = []
  for (const p of projects.value) {
    for (const b of p.branches) {
      for (const m of b.modules) {
        out.push({ project: p.name, label: `${b.environment}/${m.module}`, path: m.path })
      }
    }
  }
  return out
})

const targetGroups = computed(() => {
  const groups = new Map<string, ModuleTarget[]>()
  for (const t of moduleTargets.value) {
    const list = groups.get(t.project) ?? []
    list.push(t)
    groups.set(t.project, list)
  }
  return [...groups.entries()].map(([project, targets]) => ({ project, targets }))
})

// 默认目标优先取当前选中工作副本所属项目的第一个模块
function defaultTargetPath(): string {
  const want = props.activeProjectName
  const hit = want ? moduleTargets.value.find((t) => t.project === want) : undefined
  return (hit ?? moduleTargets.value[0])?.path ?? ''
}

onMounted(load)

async function load() {
  try {
    projects.value = await api.listProjects()
    presets.value = await api.listConfigPresets()
    // 给目标下拉一个初值，让占位文案可见而不是空白控件面
    for (const p of presets.value) {
      if (applyTarget.value[p.id] === undefined) {
        applyTarget.value[p.id] = ''
      }
    }
  } catch (e) {
    toast(e, '加载配置预设失败')
  }
}

// ===== 新建预设 =====
interface NewFileSpec {
  path: string
  mode: 'whole' | 'lines'
  ranges: [number, number][]
  // 选行用：懒加载的文件行内容与当前选区起点
  lines: string[] | null
  selStart: number | null
  pickerOpen: boolean
  // 相对 SVN 基线的本地修改行号（1-based），用于选行时高亮；null = 拿不到 diff
  changedLines: Set<number> | null
  // 折叠取行窗口，文件多时收起便于浏览
  collapsed: boolean
}

const creating = ref(false)
const newName = ref('')
const sourcePath = ref('')
const newFiles = ref<NewFileSpec[]>([])

// 来源副本的已修改文件（svn status），作为预设文件的主选单
interface SourceChange {
  path: string
  item: string
}
const sourceChanges = ref<SourceChange[]>([])
const loadingChanges = ref(false)
// 能捕获内容的版本化改动：删除/缺失无内容可取，不纳入
const CHANGED_ITEMS = new Set(['modified', 'added', 'replaced', 'conflicted'])

function startCreate() {
  creating.value = true
  newName.value = ''
  newFiles.value = []
  sourcePath.value = defaultTargetPath()
  void loadSourceChanges()
}

// 来源副本切换时重读改动，并清掉属于旧副本的已选文件
watch(sourcePath, () => {
  newFiles.value = []
  void loadSourceChanges()
})

async function loadSourceChanges() {
  sourceChanges.value = []
  if (!sourcePath.value) return
  loadingChanges.value = true
  try {
    const entries = await api.status(sourcePath.value, false)
    sourceChanges.value = entries
      .filter((e) => CHANGED_ITEMS.has(e.item))
      .map((e) => ({ path: e.path, item: e.item }))
  } catch (e) {
    toast(e, '读取来源副本改动失败')
  } finally {
    loadingChanges.value = false
  }
}

function isSelected(path: string) {
  return newFiles.value.some((f) => f.path === path)
}

function itemLabel(item: string) {
  switch (item) {
    case 'modified':
      return '已修改'
    case 'added':
      return '新增'
    case 'replaced':
      return '替换'
    case 'conflicted':
      return '冲突'
    default:
      return item
  }
}

// 勾选已修改文件：已修改类默认进指定行并自动选取修改片段；新增/取不到 diff 的回退整文件
async function toggleChange(change: SourceChange) {
  if (isSelected(change.path)) {
    removeNewFile(change.path)
    return
  }
  const spec: NewFileSpec = {
    path: change.path,
    mode: 'whole',
    ranges: [],
    lines: null,
    selStart: null,
    pickerOpen: false,
    changedLines: null,
    collapsed: false,
  }
  newFiles.value.push(spec)
  if (change.item === 'added') return
  // 必须操作数组里的响应式代理而非局部 spec，否则 setMode 的改动不触发渲染
  const f = newFiles.value[newFiles.value.length - 1]
  await setMode(f, 'lines')
  if (f.mode !== 'lines') return
  if (f.changedLines?.size) {
    pickChangedRanges(f)
  } else {
    // 拿不到改动行，留在整文件模式，避免指定行模式下无片段无法保存
    await setMode(f, 'whole')
  }
}

async function pickFiles() {
  try {
    const sel = await open({
      multiple: true,
      directory: false,
      defaultPath: sourcePath.value || undefined,
      title: '选择要纳入预设的本地配置文件',
    })
    if (!sel) return
    const paths = Array.isArray(sel) ? sel : [sel]
    for (const p of paths) {
      if (newFiles.value.some((f) => f.path === p)) continue
      newFiles.value.push({
        path: p,
        mode: 'whole',
        ranges: [],
        lines: null,
        selStart: null,
        pickerOpen: false,
        changedLines: null,
        collapsed: false,
      })
    }
  } catch (e) {
    toast(e, '选择文件失败')
  }
}

function relOf(f: string) {
  if (sourcePath.value && f.startsWith(sourcePath.value)) {
    return f.slice(sourcePath.value.length).replace(/^[/\\]/, '')
  }
  return f.split(/[/\\]/).pop() || f
}

function removeNewFile(path: string) {
  newFiles.value = newFiles.value.filter((f) => f.path !== path)
}

async function setMode(file: NewFileSpec, mode: 'whole' | 'lines') {
  file.mode = mode
  if (mode === 'whole') {
    file.pickerOpen = false
    return
  }
  file.pickerOpen = true
  if (file.lines === null) {
    try {
      const text = await api.readFileText(file.path)
      file.lines = text.split('\n').map((l) => l.replace(/\r$/, ''))
    } catch (e) {
      toast(e, '读取文件内容失败')
      file.mode = 'whole'
      file.pickerOpen = false
      return
    }
    // 高亮本地修改行：预设要捕获的通常就是这些行。拿不到 diff（文件未版本控制等）不影响选行
    try {
      file.changedLines = parseChangedLines(await api.diff(file.path))
    } catch {
      file.changedLines = null
    }
  }
}

// 从 unified diff 提取工作副本侧（+）的行号
function parseChangedLines(diffText: string): Set<number> {
  const changed = new Set<number>()
  let lineNo = 0
  let inHunk = false
  for (const raw of diffText.split('\n')) {
    const hunk = raw.match(/^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/)
    if (hunk) {
      lineNo = Number(hunk[1])
      inHunk = true
      continue
    }
    if (!inHunk) continue
    if (raw.startsWith('+++') || raw.startsWith('---')) continue
    if (raw.startsWith('+')) {
      changed.add(lineNo)
      lineNo += 1
    } else if (raw.startsWith('-') || raw.startsWith('\\')) {
      // 删除行与 "No newline" 标记不占工作副本行号
    } else if (raw.startsWith(' ') || raw === '') {
      lineNo += 1
    } else {
      // Index: / === 等分隔，离开当前 hunk
      inHunk = false
    }
  }
  return changed
}

// 把修改行聚成连续区间，一键加入片段（跳过与已选重叠的）
function pickChangedRanges(file: NewFileSpec) {
  if (!file.changedLines?.size) return
  const sorted = [...file.changedLines].sort((a, b) => a - b)
  const ranges: [number, number][] = []
  let start = sorted[0]
  let end = sorted[0]
  for (const n of sorted.slice(1)) {
    if (n === end + 1) {
      end = n
    } else {
      ranges.push([start, end])
      start = n
      end = n
    }
  }
  ranges.push([start, end])
  for (const r of ranges) {
    if (!file.ranges.some(([s, e]) => r[0] <= e && r[1] >= s)) {
      file.ranges.push(r)
    }
  }
  file.ranges.sort((a, b) => a[0] - b[0])
}

// 点第一下定起点，点第二下定终点并生成片段；再点重新开始
function clickLine(file: NewFileSpec, lineNo: number) {
  if (file.selStart === null) {
    file.selStart = lineNo
    return
  }
  const start = Math.min(file.selStart, lineNo)
  const end = Math.max(file.selStart, lineNo)
  file.selStart = null
  // 与已有片段重叠则忽略
  if (file.ranges.some(([s, e]) => start <= e && end >= s)) {
    appToast.error('行范围重叠', '与已选片段重叠，请先移除旧片段')
    return
  }
  file.ranges.push([start, end])
  file.ranges.sort((a, b) => a[0] - b[0])
}

function lineState(file: NewFileSpec, lineNo: number): 'picked' | 'pending' | '' {
  if (file.ranges.some(([s, e]) => lineNo >= s && lineNo <= e)) return 'picked'
  if (file.selStart === lineNo) return 'pending'
  return ''
}

function removeRange(file: NewFileSpec, idx: number) {
  file.ranges.splice(idx, 1)
}

// 每个文件的取行容器，用于点击片段时定位滚动
const pickerEls = new Map<string, HTMLElement>()
function setPickerEl(path: string, el: unknown) {
  if (el instanceof HTMLElement) {
    pickerEls.set(path, el)
  } else {
    pickerEls.delete(path)
  }
}

// 跳转后短暂高亮目标片段，让平滑滚动落点更醒目
const flashing = ref<{ path: string; start: number; end: number } | null>(null)
let flashTimer: ReturnType<typeof setTimeout> | null = null
function isFlashing(file: NewFileSpec, lineNo: number) {
  const f = flashing.value
  return !!f && f.path === file.path && lineNo >= f.start && lineNo <= f.end
}

// 点击片段标签：平滑滚到该片段起始行并居中，落点高亮一下
function scrollToLine(file: NewFileSpec, lineNo: number) {
  const container = pickerEls.get(file.path)
  const target = container?.children[lineNo - 1] as HTMLElement | undefined
  if (!container || !target) return
  container.scrollTo({
    top: target.offsetTop - container.clientHeight / 2 + target.clientHeight / 2,
    behavior: 'smooth',
  })
  const range = file.ranges.find(([s, e]) => lineNo >= s && lineNo <= e)
  flashing.value = { path: file.path, start: range?.[0] ?? lineNo, end: range?.[1] ?? lineNo }
  if (flashTimer) clearTimeout(flashTimer)
  flashTimer = setTimeout(() => (flashing.value = null), 1000)
}

const canSave = computed(
  () =>
    newName.value.trim().length > 0 &&
    sourcePath.value.length > 0 &&
    newFiles.value.length > 0 &&
    newFiles.value.every((f) => f.mode === 'whole' || f.ranges.length > 0),
)

async function saveCreate() {
  if (!canSave.value) return
  try {
    await api.captureConfigPreset(
      newName.value.trim(),
      sourcePath.value,
      newFiles.value.map((f) => ({
        path: f.path,
        ranges: f.mode === 'lines' ? f.ranges : [],
      })),
    )
    creating.value = false
    await load()
    appToast.success('预设已保存', newName.value.trim())
  } catch (e) {
    toast(e, '保存预设失败')
  }
}

// ===== 应用预设（先预览后确认）=====
const applyTarget = ref<Record<string, string>>({})
const previewOpen = ref(false)
const previewPlans = ref<PresetApplyPlan[]>([])
const previewPreset = ref<ConfigPreset | null>(null)
const previewTargetPath = ref('')
const applying = ref(false)

const previewHasChange = computed(() =>
  previewPlans.value.some((p) => p.action !== 'unchanged' && p.action !== 'conflict'),
)

function actionLabel(action: PresetApplyPlan['action']) {
  switch (action) {
    case 'create':
      return '新建'
    case 'overwrite':
      return '整文件覆盖'
    case 'patch':
      return '行替换'
    case 'unchanged':
      return '无变化'
    case 'conflict':
      return '冲突'
  }
}

async function openPreview(preset: ConfigPreset) {
  const target = applyTarget.value[preset.id] || defaultTargetPath()
  if (!target) {
    toast('没有可应用的目标工作副本', '无法应用')
    return
  }
  try {
    previewPlans.value = await api.previewConfigPreset(preset.id, target)
    previewPreset.value = preset
    previewTargetPath.value = target
    previewOpen.value = true
  } catch (e) {
    toast(e, '生成应用预览失败')
  }
}

async function confirmApply() {
  if (!previewPreset.value || !previewTargetPath.value) return
  applying.value = true
  try {
    const plans = await api.applyConfigPreset(previewPreset.value.id, previewTargetPath.value)
    const written = plans.filter((p) => p.action !== 'unchanged' && p.action !== 'conflict')
    const conflicts = plans.filter((p) => p.action === 'conflict')
    previewOpen.value = false
    if (conflicts.length) {
      appToast.error(
        `${conflicts.length} 个文件未应用`,
        conflicts.map((c) => `${c.relPath}：${c.detail}`).join('\n'),
      )
    }
    if (written.length) {
      appToast.success('已应用', `${written.length} 个文件已写入`)
    } else if (!conflicts.length) {
      appToast.success('已应用', '内容已一致，无需写入')
    }
  } catch (e) {
    toast(e, '应用预设失败')
  } finally {
    applying.value = false
  }
}

async function removePreset(preset: ConfigPreset) {
  const ok = await confirm({
    title: '删除预设',
    content: `删除预设「${preset.name}」？`,
    confirmText: '删除',
    destructive: true,
  })
  if (!ok) return
  try {
    await api.deleteConfigPreset(preset.id)
    presets.value = await api.listConfigPresets()
  } catch (e) {
    toast(e, '删除失败')
  }
}

function fileSummary(preset: ConfigPreset) {
  const fragments = preset.files.reduce((n, f) => n + (f.fragments?.length ?? 0), 0)
  return fragments > 0
    ? `${preset.files.length} 个文件 · ${fragments} 个行片段`
    : `${preset.files.length} 个文件`
}
</script>

<template>
  <div class="preset-view">
    <div class="scroll">
      <div class="head-row">
        <span class="title">配置预设</span>
        <span class="subtitle">全局统一，所有项目通用</span>
        <span class="spacer" />
        <Button size="xs" @click="startCreate">
          <FilePlus2 class="icon-xs" /> 新建预设
        </Button>
      </div>

      <p class="intro">
        把 application-dev.yml、ConstantsSystem.java 等本地开发配置捕获为预设：可以只选取文件中的某几行，
        应用时按上下文定位替换那几行，而不是整文件覆盖。预设全局共享，任何项目都能直接套用——
        目录结构不同（如 Java 包路径不一样）时会按文件名自动定位目标文件，应用前的预览里会标明定位结果。
      </p>

      <!-- 新建表单 -->
      <div v-if="creating" class="create-card">
        <div class="form-row">
          <span class="label">预设名称</span>
          <Input v-model="newName" placeholder="如：本地开发配置" />
        </div>
        <div class="form-row">
          <span class="label">来源副本</span>
          <select v-model="sourcePath" class="ui-select">
            <optgroup v-for="g in targetGroups" :key="g.project" :label="g.project">
              <option v-for="t in g.targets" :key="t.path" :value="t.path">
                {{ g.project }} / {{ t.label }}
              </option>
            </optgroup>
          </select>
        </div>
        <div class="form-row">
          <span class="label">文件</span>
          <span class="muted">来自来源副本的改动{{ newFiles.length ? ` · 已选 ${newFiles.length} 个` : '' }}</span>
          <span class="spacer" />
          <Button size="xs" variant="ghost" :disabled="!sourcePath" @click="pickFiles">
            从磁盘选其他文件…
          </Button>
        </div>

        <div v-if="loadingChanges" class="muted change-hint">正在读取来源副本改动…</div>
        <div v-else-if="sourceChanges.length" class="change-list">
          <label v-for="c in sourceChanges" :key="c.path" class="change-row">
            <input type="checkbox" :checked="isSelected(c.path)" @change="toggleChange(c)" />
            <span class="mono change-rel" :title="c.path">{{ relOf(c.path) }}</span>
            <span :class="['ci-badge', `ci-${c.item}`]">{{ itemLabel(c.item) }}</span>
          </label>
        </div>
        <div v-else-if="sourcePath" class="muted change-hint">来源副本没有已修改的文件</div>

        <!-- 每个文件：整文件 / 指定行 -->
        <div v-for="file in newFiles" :key="file.path" class="file-card">
          <div class="file-head">
            <FileText class="p-icon" />
            <span class="mono file-rel" :title="file.path">{{ relOf(file.path) }}</span>
            <span class="spacer" />
            <div class="mode-seg">
              <button
                type="button"
                :class="['mode-btn', { active: file.mode === 'whole' }]"
                @click="setMode(file, 'whole')"
              >
                整文件
              </button>
              <button
                type="button"
                :class="['mode-btn', { active: file.mode === 'lines' }]"
                @click="setMode(file, 'lines')"
              >
                指定行
              </button>
            </div>
            <Button
              v-if="file.mode === 'lines'"
              size="xs"
              variant="ghost"
              :title="file.collapsed ? '展开' : '折叠'"
              @click="file.collapsed = !file.collapsed"
            >
              <component :is="file.collapsed ? ChevronRight : ChevronDown" class="icon-xs" />
            </Button>
            <Button size="xs" variant="ghost" @click="removeNewFile(file.path)">
              <X class="icon-xs" />
            </Button>
          </div>

          <template v-if="file.mode === 'lines'">
            <Transition name="summary">
              <div v-if="file.collapsed" class="muted collapsed-summary">
                {{ file.ranges.length ? `${file.ranges.length} 个片段（已折叠）` : '未选择片段（已折叠）' }}
              </div>
            </Transition>

            <div class="lines-wrap" :class="{ collapsed: file.collapsed }">
              <div class="lines-inner">
                <div class="range-chips">
                  <span v-if="!file.ranges.length" class="muted">
                    {{ file.selStart === null ? '点击下方行号选择起始行' : `起点第 ${file.selStart} 行，再点结束行` }}
                  </span>
                  <span v-for="(r, i) in file.ranges" :key="`${r[0]}-${r[1]}`" class="chip">
                    <button type="button" class="chip-label" @click="scrollToLine(file, r[0])">
                      {{ r[0] === r[1] ? `第 ${r[0]} 行` : `${r[0]}-${r[1]} 行` }}
                    </button>
                    <button type="button" class="chip-x" @click="removeRange(file, i)">
                      <X class="icon-xxs" />
                    </button>
                  </span>
                  <template v-if="file.changedLines?.size">
                    <span class="spacer" />
                    <span class="changed-legend"><i class="legend-bar" />本地修改行</span>
                    <Button size="xs" variant="ghost" @click="pickChangedRanges(file)">
                      选取修改行
                    </Button>
                  </template>
                </div>
                <div v-if="file.lines" class="line-picker mono" :ref="(el) => setPickerEl(file.path, el)">
                  <div
                    v-for="(line, idx) in file.lines"
                    :key="idx"
                    :class="['pick-line', lineState(file, idx + 1), { changed: file.changedLines?.has(idx + 1), flash: isFlashing(file, idx + 1) }]"
                    @click="clickLine(file, idx + 1)"
                  >
                    <span class="ln">{{ idx + 1 }}</span>
                    <span class="lc">{{ line || ' ' }}</span>
                  </div>
                </div>
              </div>
            </div>
          </template>
        </div>

        <div class="form-actions">
          <Button size="xs" variant="ghost" @click="creating = false">取消</Button>
          <Button size="xs" :disabled="!canSave" @click="saveCreate">保存预设</Button>
        </div>
      </div>

      <!-- 预设列表 -->
      <EmptyState v-if="!creating && presets.length === 0" description="还没有配置预设" />
      <div v-for="preset in presets" :key="preset.id" class="preset-card">
        <div class="preset-head">
          <FileText class="p-icon" />
          <span class="p-name">{{ preset.name }}</span>
          <span class="p-count">{{ fileSummary(preset) }}</span>
          <span class="spacer" />
          <select v-model="applyTarget[preset.id]" class="ui-select small">
            <option value="" disabled hidden>选择应用目标…</option>
            <optgroup v-for="g in targetGroups" :key="g.project" :label="g.project">
              <option v-for="t in g.targets" :key="t.path" :value="t.path">
                {{ g.project }} / {{ t.label }}
              </option>
            </optgroup>
          </select>
          <Button size="xs" @click="openPreview(preset)">
            <Wand2 class="icon-xs" /> 预览应用
          </Button>
          <Button size="xs" variant="ghost" @click="removePreset(preset)">
            <Trash2 class="icon-xs" />
          </Button>
        </div>
        <ul class="file-list">
          <li v-for="f in preset.files" :key="f.relPath" class="mono">
            {{ f.relPath }}
            <span v-if="f.fragments?.length" class="frag-tag">
              <ListOrdered class="icon-xxs" />
              {{ f.fragments.map((fr) => (fr.startLine === fr.endLine ? `${fr.startLine}` : `${fr.startLine}-${fr.endLine}`)).join(', ') }} 行
            </span>
            <span v-else class="frag-tag whole">整文件</span>
          </li>
        </ul>
      </div>
    </div>

    <!-- 应用前预览确认 -->
    <Dialog v-model:open="previewOpen">
      <DialogContent class="preview-dialog">
        <DialogHeader>
          <DialogTitle>应用「{{ previewPreset?.name }}」</DialogTitle>
        </DialogHeader>
        <div class="preview-target mono">→ {{ previewTargetPath }}</div>
        <div class="preview-body">
          <div v-for="plan in previewPlans" :key="plan.relPath" class="plan-item">
            <div class="plan-head">
              <span :class="['plan-badge', `act-${plan.action}`]">{{ actionLabel(plan.action) }}</span>
              <span class="mono plan-path">{{ plan.relPath }}</span>
            </div>
            <div class="plan-detail">{{ plan.detail }}</div>
            <div v-if="plan.action === 'patch'" class="plan-diff mono">
              <div v-for="(l, i) in plan.oldLines" :key="`o${i}`" class="d-line d-old">- {{ l }}</div>
              <div v-for="(l, i) in plan.newLines" :key="`n${i}`" class="d-line d-new">+ {{ l }}</div>
            </div>
          </div>
        </div>
        <DialogFooter>
          <div class="modal-actions">
            <Button variant="outline" @click="previewOpen = false">取消</Button>
            <Button :disabled="!previewHasChange || applying" @click="confirmApply">
              {{ applying ? '应用中…' : '确认应用' }}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<style scoped>
.preset-view {
  height: 100%;
  min-height: 0;
  background: var(--mat-content);
  overflow: hidden;
  display: flex;
}
.scroll {
  flex: 1;
  overflow: auto;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
/* 容器高度确定时 flex 会压缩子项去适配，导致溢出被吞进内层 line-picker 而外层不滚动；
   禁止子项收缩，让内容保持自然高度，外层才会出现滚动条 */
.scroll > * {
  flex-shrink: 0;
}
.head-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.title {
  font-size: var(--fs-callout);
  font-weight: 600;
  color: var(--fg);
}
.subtitle {
  font-size: var(--fs-caption);
  color: var(--fg-subtle);
}
.label {
  font-size: var(--fs-callout);
  color: var(--fg-muted);
  min-width: 60px;
  font-weight: 500;
}
/* 外观由全局 .ui-select（macOS 弹出按钮）提供，这里只管布局 */
.ui-select {
  flex: 1;
  min-width: 0;
}
.ui-select.small {
  flex: 0 0 auto;
  max-width: 220px;
}
.intro {
  font-size: var(--fs-caption);
  color: var(--fg-subtle);
  margin: 0;
}
.create-card,
.preset-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border-radius: 8px;
  background: var(--mat-toolbar);
  box-shadow: var(--stroke-control);
}
.form-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.muted {
  font-size: var(--fs-caption);
  color: var(--fg-subtle);
}
.form-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

/* ===== 新建：来源副本改动选单 ===== */
.change-hint {
  padding: 2px 0 2px 70px;
}
.change-list {
  display: flex;
  flex-direction: column;
  gap: 1px;
  max-height: 220px;
  overflow: auto;
  border-radius: 6px;
  background: var(--mat-content);
  box-shadow: var(--stroke-control);
  padding: 4px;
}
.change-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 3px 6px;
  border-radius: 4px;
  cursor: pointer;
}
.change-row:hover {
  background: var(--accent-soft);
}
.change-rel {
  flex: 1;
  min-width: 0;
  font-size: var(--fs-mono);
  color: var(--fg);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ci-badge {
  flex: none;
  font-size: var(--fs-caption);
  font-weight: 600;
  padding: 1px 7px;
  border-radius: var(--radius-pill);
}
.ci-modified {
  color: var(--accent);
  background: var(--accent-soft);
}
.ci-added {
  color: var(--success, #30a46c);
  background: color-mix(in srgb, var(--success, #30a46c) 14%, transparent);
}
.ci-replaced {
  color: var(--warning, #f5a623);
  background: color-mix(in srgb, var(--warning, #f5a623) 16%, transparent);
}
.ci-conflicted {
  color: var(--danger);
  background: color-mix(in srgb, var(--danger) 14%, transparent);
}

/* ===== 新建：单个文件卡片 ===== */
.file-card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px 10px;
  border-radius: 6px;
  background: var(--mat-elevated);
  box-shadow: var(--stroke-control);
}
.file-head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.file-rel {
  font-size: var(--fs-mono);
  color: var(--fg);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mode-seg {
  display: flex;
  gap: 2px;
  padding: 2px;
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.06);
}
.dark .mode-seg {
  background: rgba(255, 255, 255, 0.05);
}
.mode-btn {
  border: 0;
  background: transparent;
  font-size: var(--fs-caption);
  color: var(--fg-muted);
  padding: 2px 8px;
  border-radius: 4px;
}
.mode-btn.active {
  background: var(--mat-content);
  color: var(--fg-strong);
  box-shadow: var(--stroke-control);
}
.collapsed-summary {
  padding: 2px 2px 0;
}
/* 折叠摘要淡入淡出 */
.summary-enter-active,
.summary-leave-active {
  transition: opacity 0.2s ease;
}
.summary-enter-from,
.summary-leave-to {
  opacity: 0;
}

/* 折叠动画：用 grid 行高 0fr↔1fr 平滑收放，配合淡出更有质感 */
.lines-wrap {
  display: grid;
  grid-template-rows: 1fr;
  opacity: 1;
  transition:
    grid-template-rows 0.32s cubic-bezier(0.16, 1, 0.3, 1),
    opacity 0.24s ease;
}
.lines-wrap.collapsed {
  grid-template-rows: 0fr;
  opacity: 0;
}
.lines-inner {
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.range-chips {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
}
.chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: var(--fs-caption);
  color: var(--accent);
  background: var(--accent-soft);
  padding: 1px 4px 1px 8px;
  border-radius: var(--radius-pill);
}
.chip-label {
  border: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  padding: 0;
  cursor: pointer;
}
.chip-label:hover {
  text-decoration: underline;
}
.chip-x {
  border: 0;
  background: transparent;
  display: inline-flex;
  color: inherit;
  padding: 2px;
}
.line-picker {
  position: relative;
  max-height: 260px;
  overflow: auto;
  border-radius: 6px;
  background: var(--mat-content);
  box-shadow: var(--stroke-control);
  font-size: var(--fs-mono);
}
.pick-line {
  display: flex;
  gap: 10px;
  padding: 0 8px;
  line-height: 1.7;
  white-space: pre;
  cursor: default;
}
.pick-line:hover {
  background: var(--accent-soft);
}
/* 本地修改行：左侧琥珀色标记条 + 行号着色（在 picked 之前定义，选中态优先） */
.pick-line.changed {
  box-shadow: inset 2px 0 0 var(--warning);
  background: color-mix(in srgb, var(--warning) 7%, transparent);
}
.pick-line.changed .ln {
  color: var(--warning);
  font-weight: 600;
}
.changed-legend {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: var(--fs-caption);
  color: var(--fg-subtle);
}
.legend-bar {
  width: 3px;
  height: 12px;
  border-radius: 2px;
  background: var(--warning);
}
.pick-line.picked {
  background: color-mix(in srgb, var(--accent) 18%, transparent);
}
.pick-line.pending {
  background: color-mix(in srgb, var(--warning, #f5a623) 22%, transparent);
}
/* 跳转落点脉冲：盖过 picked 背景闪一下再回落 */
.pick-line.flash {
  animation: line-flash 1s ease-out;
}
@keyframes line-flash {
  0%,
  20% {
    background: color-mix(in srgb, var(--accent) 42%, transparent);
  }
  100% {
    background: transparent;
  }
}
.ln {
  flex: none;
  min-width: 34px;
  text-align: right;
  color: var(--fg-subtle);
  user-select: none;
}
.lc {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--fg-muted);
}

/* ===== 预设列表 ===== */
.preset-head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.p-icon {
  width: 15px;
  height: 15px;
  color: var(--accent);
  flex: none;
}
.p-name {
  font-size: var(--fs-callout);
  font-weight: 600;
  color: var(--fg);
}
.p-count {
  font-size: var(--fs-caption);
  color: var(--fg-subtle);
}
.spacer {
  flex: 1;
}
.file-list {
  margin: 0;
  padding: 6px 0 0 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.file-list li {
  font-size: var(--fs-mono);
  color: var(--fg-muted);
  word-break: break-all;
  padding-left: 22px;
  display: flex;
  align-items: center;
  gap: 8px;
}
.frag-tag {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: var(--fs-caption);
  color: var(--accent);
  flex: none;
}
.frag-tag.whole {
  color: var(--fg-subtle);
}

/* ===== 预览对话框 ===== */
/* .preview-dialog 本身见文件末尾的全局 style 块：DialogContent 经 Teleport 渲染到 body，
   拿不到本组件的 scoped data-v，scoped 规则匹配不到，只能用全局选择器 */
.preview-target {
  font-size: var(--fs-caption);
  color: var(--fg-muted);
  word-break: break-all;
}
.preview-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 4px 0;
}
.plan-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 10px;
  border-radius: 6px;
  background: var(--mat-elevated);
  box-shadow: var(--stroke-control);
}
.plan-head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.plan-badge {
  font-size: var(--fs-caption);
  font-weight: 600;
  padding: 1px 7px;
  border-radius: var(--radius-pill);
  flex: none;
}
.act-create {
  color: var(--success, #30a46c);
  background: color-mix(in srgb, var(--success, #30a46c) 14%, transparent);
}
.act-overwrite {
  color: var(--warning, #f5a623);
  background: color-mix(in srgb, var(--warning, #f5a623) 16%, transparent);
}
.act-patch {
  color: var(--accent);
  background: var(--accent-soft);
}
.act-unchanged {
  color: var(--fg-subtle);
  background: color-mix(in srgb, var(--fg-subtle) 12%, transparent);
}
.act-conflict {
  color: var(--danger);
  background: color-mix(in srgb, var(--danger) 14%, transparent);
}
.plan-path {
  font-size: var(--fs-mono);
  color: var(--fg);
  word-break: break-all;
}
.plan-detail {
  font-size: var(--fs-caption);
  color: var(--fg-muted);
}
.plan-diff {
  border-radius: 6px;
  background: var(--mat-content);
  box-shadow: var(--stroke-control);
  padding: 6px 8px;
  overflow-x: auto;
  font-size: var(--fs-mono);
}
.d-line {
  white-space: pre;
  line-height: 1.6;
}
.d-old {
  color: var(--danger);
}
.d-new {
  color: var(--success, #30a46c);
}
.modal-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}

.icon-xs {
  width: 13px;
  height: 13px;
}
.icon-xxs {
  width: 11px;
  height: 11px;
}
</style>

<!-- 全局：DialogContent 被 Teleport 到 body，scoped 规则匹配不到，需用全局选择器。
     叠加 .dialog-content + [role] 提高优先级，压过 DialogContent 默认的 display:grid / max-width，
     否则内容溢出视口且 .preview-body 无法滚动 -->
<style>
.dialog-content.preview-dialog[role='dialog'] {
  width: 620px;
  max-width: calc(100vw - 48px);
  max-height: calc(100vh - 80px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
</style>
