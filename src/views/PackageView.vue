<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Package, RefreshCw, FolderOpen, CheckCircle2 } from 'lucide-vue-next'

import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Badge } from '@/components/ui/badge'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import LoadingSpinner from '@/components/ui-local/LoadingSpinner.vue'
import { api } from '../api/svn'
import { useErrorToast } from '../composables/use-error-toast'
import { useAppToast } from '../composables/use-app-toast'
import type { PackageBuildResult, PackageRevision, PackageZipResult, Project } from '../types/svn'

const props = defineProps<{ activeProjectName?: string | null }>()

const toast = useErrorToast()
const appToast = useAppToast()

const projects = ref<Project[]>([])
const projectName = ref<string>('')

// 该项目下所有 rest 模块（develop/rest、produce/rest 等）
interface RestTarget {
  label: string
  path: string
}
const restTargets = ref<RestTarget[]>([])
const restPath = ref<string>('')

const requirementName = ref('')
const requirementDesc = ref('')
const hasDb = ref(false)
const hasUrl = ref(false)
const hasFrontend = ref(false)

const loadingRevisions = ref(false)
const revisions = ref<PackageRevision[]>([])
const selected = ref<Set<number>>(new Set())
const filter = ref('')
const fetched = ref(false)

const building = ref(false)
const buildResult = ref<PackageBuildResult | null>(null)
const zipping = ref(false)
const zipResult = ref<PackageZipResult | null>(null)
const committing = ref(false)
const committed = ref(false)

const filteredRevisions = computed(() => {
  const kw = filter.value.trim().toLowerCase()
  if (!kw) return revisions.value
  return revisions.value.filter(
    (r) =>
      String(r.revision).includes(kw) ||
      (r.author ?? '').toLowerCase().includes(kw) ||
      r.message.toLowerCase().includes(kw),
  )
})
const selectedCount = computed(() => selected.value.size)
const canBuild = computed(
  () => !!restPath.value && requirementName.value.trim().length > 0 && !building.value,
)

onMounted(loadProjects)

async function loadProjects() {
  try {
    projects.value = await api.listProjects()
    const want = props.activeProjectName
    const match = want && projects.value.find((p) => p.name === want)
    projectName.value = match ? want! : projects.value[0]?.name ?? ''
    onProjectChange()
  } catch (e) {
    toast(e, '加载项目失败')
  }
}

watch(
  () => props.activeProjectName,
  (name) => {
    if (name && projects.value.some((p) => p.name === name) && name !== projectName.value) {
      projectName.value = name
      onProjectChange()
    }
  },
)

function onProjectChange() {
  const project = projects.value.find((p) => p.name === projectName.value)
  const targets: RestTarget[] = []
  for (const b of project?.branches ?? []) {
    const rest = b.modules.find((m) => m.module.toLowerCase() === 'rest')
    if (rest) targets.push({ label: `${b.environment}/rest`, path: rest.path })
  }
  restTargets.value = targets
  restPath.value = targets[0]?.path ?? ''
  resetAfterConfig()
}

function resetAfterConfig() {
  revisions.value = []
  selected.value = new Set()
  filter.value = ''
  fetched.value = false
  buildResult.value = null
  zipResult.value = null
  committed.value = false
}

async function fetchRevisions() {
  if (!restPath.value) return
  loadingRevisions.value = true
  fetched.value = false
  try {
    revisions.value = await api.packageFetchRevisions(restPath.value, 20)
    selected.value = new Set()
    fetched.value = true
  } catch (e) {
    toast(e, '拉取版本失败')
  } finally {
    loadingRevisions.value = false
  }
}

function toggleRev(rev: number, checked: boolean | 'indeterminate') {
  const next = new Set(selected.value)
  if (checked === true) next.add(rev)
  else next.delete(rev)
  selected.value = next
}

async function build() {
  if (!canBuild.value) return
  building.value = true
  buildResult.value = null
  zipResult.value = null
  committed.value = false
  try {
    const revs = [...selected.value].sort((a, b) => a - b)
    buildResult.value = await api.packageBuild(
      restPath.value,
      {
        requirementName: requirementName.value.trim(),
        requirementDesc: requirementDesc.value.trim(),
        hasDb: hasDb.value,
        hasUrl: hasUrl.value,
      },
      revs,
    )
    // 不需要前端包时可直接打 ZIP
    if (!hasFrontend.value) await makeZip()
  } catch (e) {
    toast(e, '构建增量包失败')
  } finally {
    building.value = false
  }
}

async function makeZip() {
  if (!buildResult.value) return
  zipping.value = true
  try {
    zipResult.value = await api.packageMakeZip(
      buildResult.value.baseDir,
      requirementName.value.trim(),
    )
  } catch (e) {
    toast(e, '打包 ZIP 失败')
  } finally {
    zipping.value = false
  }
}

async function commitVersion() {
  if (!buildResult.value || !restPath.value) return
  committing.value = true
  try {
    await api.packageCommitVersion(restPath.value, buildResult.value.version)
    committed.value = true
    appToast.success('version 已提交', buildResult.value.version)
  } catch (e) {
    toast(e, '提交 version 失败')
  } finally {
    committing.value = false
  }
}

async function reveal(path: string) {
  try {
    await api.revealInFileManager(path)
  } catch (e) {
    toast(e, '打开目录失败')
  }
}

function fmtSize(bytes: number) {
  return bytes < 1024 * 1024
    ? `${(bytes / 1024).toFixed(1)} KB`
    : `${(bytes / 1024 / 1024).toFixed(2)} MB`
}
</script>

<template>
  <div class="package-view">
    <div class="scroll">
      <!-- 配置 -->
      <section class="block">
        <div class="config-row">
          <span class="label">项目</span>
          <select v-model="projectName" class="ui-select" @change="onProjectChange">
            <option v-for="p in projects" :key="p.name" :value="p.name">{{ p.name }}</option>
          </select>
        </div>
        <div class="config-row">
          <span class="label">打包分支</span>
          <select v-model="restPath" class="ui-select" :disabled="restTargets.length === 0" @change="resetAfterConfig">
            <option v-for="t in restTargets" :key="t.path" :value="t.path">{{ t.label }}</option>
          </select>
        </div>
        <div v-if="restTargets.length === 0" class="hint">该项目未识别到 rest 模块</div>
        <div class="config-row">
          <span class="label">需求名称</span>
          <Input v-model="requirementName" placeholder="如：推送管理修复" />
        </div>
        <div class="config-row">
          <span class="label">需求描述</span>
          <Input v-model="requirementDesc" placeholder="留空则用需求名称" />
        </div>
        <div class="opts">
          <label class="opt"><Checkbox :model-value="hasDb" @update:model-value="(v: boolean | 'indeterminate') => (hasDb = v === true)" /> 含数据库脚本</label>
          <label class="opt"><Checkbox :model-value="hasUrl" @update:model-value="(v: boolean | 'indeterminate') => (hasUrl = v === true)" /> 含浏览器 URL</label>
          <label class="opt"><Checkbox :model-value="hasFrontend" @update:model-value="(v: boolean | 'indeterminate') => (hasFrontend = v === true)" /> 含前端包</label>
        </div>
      </section>

      <!-- 版本选择 -->
      <section class="block">
        <div class="rev-toolbar">
          <span class="label">增量版本</span>
          <Button size="xs" :disabled="loadingRevisions || !restPath" @click="fetchRevisions">
            <RefreshCw class="icon-xs" :class="{ spin: loadingRevisions }" />
            {{ fetched ? '重新拉取' : '拉取最近提交' }}
          </Button>
          <input v-if="fetched && revisions.length" v-model="filter" class="native-input" placeholder="过滤" />
          <span class="spacer" />
          <Badge v-if="fetched && revisions.length" variant="secondary">已选 {{ selectedCount }}</Badge>
        </div>
        <div class="rev-list-wrap">
          <div v-if="loadingRevisions" class="hint center"><LoadingSpinner /> 拉取中…</div>
          <EmptyState v-else-if="!fetched" description="选好分支后拉取最近提交，勾选本次需求涉及的版本" />
          <EmptyState v-else-if="revisions.length === 0" description="没有提交记录" />
          <div v-else class="rev-list">
            <label
              v-for="r in filteredRevisions"
              :key="r.revision"
              :class="['rev-item', { checked: selected.has(r.revision) }]"
            >
              <Checkbox
                :model-value="selected.has(r.revision)"
                @update:model-value="(v: boolean | 'indeterminate') => toggleRev(r.revision, v)"
              />
              <div class="rev-meta">
                <div class="rev-head">
                  <span class="rev-num mono">r{{ r.revision }}</span>
                  <span class="rev-author">{{ r.author || '—' }}</span>
                  <span class="rev-date">{{ (r.date || '').slice(0, 19).replace('T', ' ') }}</span>
                </div>
                <div class="rev-msg">{{ r.message }}</div>
              </div>
            </label>
          </div>
        </div>
        <p class="tip">提示：先在 IDEA 完成 Maven package，再构建增量包。不勾选版本则只复制全量产物而不提取增量。</p>
      </section>

      <!-- 构建 -->
      <section class="block actions-block">
        <Button :disabled="!canBuild" @click="build">
          <Package class="icon-xs" />
          {{ building ? '构建中…' : '构建增量包' }}
        </Button>
      </section>

      <!-- 构建结果 -->
      <section v-if="buildResult" class="block result-block">
        <div class="result-head">
          <CheckCircle2 class="ok-icon" />
          <span>构建完成 · 增量 {{ buildResult.copiedCount }} 个文件 · 版本 {{ buildResult.version }}</span>
          <Button size="xs" variant="ghost" @click="reveal(buildResult.baseDir)">
            <FolderOpen class="icon-xs" /> 打开提供包
          </Button>
        </div>
        <Textarea :model-value="buildResult.log.join('\n')" class="build-log mono" readonly />
        <div v-if="buildResult.notFound.length" class="not-found">
          未匹配到 {{ buildResult.notFound.length }} 个变更（可能是删除项或非 rest/src 路径）：
          <code v-for="nf in buildResult.notFound.slice(0, 10)" :key="nf">{{ nf }}</code>
        </div>

        <!-- 前端包等待 -->
        <div v-if="hasFrontend && !zipResult" class="frontend-wait">
          <span>请把前端包放入 <code>{{ buildResult.frontDir }}</code>，完成后打包 ZIP。</span>
          <div class="row-actions">
            <Button size="xs" variant="ghost" @click="reveal(buildResult.frontDir)">
              <FolderOpen class="icon-xs" /> 打开前端目录
            </Button>
            <Button size="xs" :disabled="zipping" @click="makeZip">
              {{ zipping ? '打包中…' : '打包 ZIP' }}
            </Button>
          </div>
        </div>
      </section>

      <!-- ZIP 结果 -->
      <section v-if="zipResult" class="block result-block">
        <div class="result-head">
          <CheckCircle2 class="ok-icon" />
          <span>ZIP 完成 · {{ fmtSize(zipResult.size) }}</span>
          <Button size="xs" variant="ghost" @click="reveal(zipResult.zipPath)">
            <FolderOpen class="icon-xs" /> 定位 ZIP
          </Button>
        </div>
        <code class="zip-path mono">{{ zipResult.zipPath }}</code>
        <div class="commit-version">
          <span>把版本号写回分支并提交：<b>{{ buildResult?.version }}</b></span>
          <Button size="xs" :disabled="committing || committed" @click="commitVersion">
            {{ committed ? '已提交' : committing ? '提交中…' : '提交 version' }}
          </Button>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.package-view {
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
.block {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border-radius: 8px;
  background: var(--mat-toolbar);
  box-shadow: var(--stroke-control);
}
.config-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.label {
  font-size: var(--fs-callout);
  color: var(--fg-muted);
  min-width: 64px;
  font-weight: 500;
}
/* select 外观由全局 .ui-select（macOS 弹出按钮）提供，这里只管布局 */
.ui-select {
  flex: 1;
  min-width: 0;
}
.native-input {
  height: 26px;
  padding: 0 8px;
  font-size: var(--fs-callout);
  border-radius: 6px;
  background: var(--mat-elevated);
  box-shadow: var(--stroke-control);
  color: var(--fg);
  border: 0;
  outline: none;
}
.native-input {
  width: 120px;
}
.opts {
  display: flex;
  gap: 18px;
  padding-top: 2px;
}
.opt {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--fs-callout);
  color: var(--fg);
  cursor: pointer;
}
.rev-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
}
.spacer {
  flex: 1;
}
.rev-list-wrap {
  max-height: 280px;
  overflow: auto;
  border-radius: 6px;
  background: var(--mat-content);
  box-shadow: var(--stroke-control);
}
.rev-list {
  display: flex;
  flex-direction: column;
}
.rev-item {
  display: flex;
  gap: 9px;
  padding: 7px 10px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
  cursor: pointer;
  align-items: flex-start;
}
.rev-item.checked {
  background: var(--accent-soft);
}
.rev-meta {
  min-width: 0;
  flex: 1;
}
.rev-head {
  display: flex;
  gap: 10px;
  align-items: baseline;
  font-size: var(--fs-caption);
}
.rev-num {
  color: var(--accent);
  font-weight: 600;
}
.rev-author {
  color: var(--fg);
}
.rev-date {
  color: var(--fg-subtle);
}
.rev-msg {
  font-size: var(--fs-callout);
  color: var(--fg-muted);
  white-space: pre-wrap;
  word-break: break-word;
  margin-top: 2px;
}
.tip {
  font-size: var(--fs-caption);
  color: var(--fg-subtle);
  margin: 0;
}
.actions-block {
  align-items: flex-start;
}
.result-block {
  gap: 10px;
}
.result-head {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--fs-callout);
  color: var(--fg);
}
.ok-icon {
  width: 16px;
  height: 16px;
  color: var(--success, #22c55e);
}
.build-log {
  min-height: 90px;
  max-height: 180px;
  font-size: var(--fs-mono);
}
.not-found {
  font-size: var(--fs-caption);
  color: var(--fg-muted);
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.not-found code,
.zip-path,
.frontend-wait code,
.commit-version code {
  font-size: var(--fs-mono);
  color: var(--fg-subtle);
  word-break: break-all;
}
.frontend-wait {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px;
  border-radius: 6px;
  background: var(--accent-soft);
  font-size: var(--fs-callout);
  color: var(--fg);
}
.row-actions {
  display: flex;
  gap: 8px;
}
.commit-version {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  font-size: var(--fs-callout);
  color: var(--fg);
  padding-top: 4px;
  border-top: var(--hairline) solid var(--stroke-soft);
}
.icon-xs {
  width: 13px;
  height: 13px;
}
.icon-xs.spin {
  animation: spin 0.8s linear infinite;
}
@keyframes spin {
  to { transform: rotate(360deg); }
}
.hint {
  font-size: var(--fs-callout);
  color: var(--fg-muted);
}
.hint.center {
  display: flex;
  gap: 6px;
  justify-content: center;
  padding: 24px;
  align-items: center;
}
</style>
