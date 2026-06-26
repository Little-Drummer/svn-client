<script setup lang="ts">
import { onMounted, ref } from 'vue'
import {
  GitBranch,
  GitMerge,
  Package,
  FileCog,
  RefreshCw,
  FolderGit2,
  CircleDot,
} from 'lucide-vue-next'

import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import { api } from '../api/svn'
import { useErrorToast } from '../composables/use-error-toast'
import type { Project } from '../types/svn'

const emit = defineEmits<{
  navigate: [tab: 'merge' | 'package' | 'config', projectName: string]
}>()

const toast = useErrorToast()

const projects = ref<Project[]>([])
const loading = ref(false)

// 每个 rest 模块的本地改动检查结果：path -> 变更文件数（-1 表示检查失败）
const dirtyCounts = ref<Record<string, number>>({})
const checking = ref<Record<string, boolean>>({})

// 工作流步骤图例（静态，帮助用户对齐流程心智）
const STEPS = ['拉取', '配置', '开发', '合并', '提测', '打包']

onMounted(load)

async function load() {
  loading.value = true
  try {
    projects.value = await api.listProjects()
  } catch (e) {
    toast(e, '加载项目失败')
  } finally {
    loading.value = false
  }
}

function restModules(project: Project) {
  const out: { label: string; path: string }[] = []
  for (const b of project.branches) {
    const rest = b.modules.find((m) => m.module.toLowerCase() === 'rest')
    if (rest) out.push({ label: `${b.environment}/rest`, path: rest.path })
  }
  return out
}

function branchSummary(project: Project) {
  return project.branches.map((b) => ({
    env: b.environment,
    modules: b.modules.map((m) => m.module),
  }))
}

async function checkProject(project: Project) {
  for (const rest of restModules(project)) {
    checking.value = { ...checking.value, [rest.path]: true }
    try {
      const entries = await api.status(rest.path, false)
      const changed = entries.filter((e) => e.item !== 'normal' && e.item !== 'unversioned').length
      dirtyCounts.value = { ...dirtyCounts.value, [rest.path]: changed }
    } catch {
      dirtyCounts.value = { ...dirtyCounts.value, [rest.path]: -1 }
    } finally {
      checking.value = { ...checking.value, [rest.path]: false }
    }
  }
}

function dirtyLabel(path: string): { text: string; tone: 'clean' | 'dirty' | 'unknown' | 'error' } {
  const c = dirtyCounts.value[path]
  if (c === undefined) return { text: '未检查', tone: 'unknown' }
  if (c === -1) return { text: '检查失败', tone: 'error' }
  if (c === 0) return { text: '无改动', tone: 'clean' }
  return { text: `${c} 处改动`, tone: 'dirty' }
}
</script>

<template>
  <div class="board-view">
    <div class="board-head">
      <span class="title">项目总览</span>
      <div class="steps">
        <template v-for="(s, i) in STEPS" :key="s">
          <span class="step">{{ s }}</span>
          <span v-if="i < STEPS.length - 1" class="step-arrow">→</span>
        </template>
      </div>
      <span class="spacer" />
      <Button size="xs" variant="ghost" :disabled="loading" @click="load">
        <RefreshCw class="icon-xs" :class="{ spin: loading }" /> 刷新
      </Button>
    </div>

    <div class="board-scroll">
      <EmptyState
        v-if="!loading && projects.length === 0"
        description="还没有识别到项目。在左侧用「项目」按钮扫描一个项目根目录。"
      />
      <div v-else class="cards">
        <div v-for="project in projects" :key="project.name" class="proj-card">
          <div class="card-head">
            <FolderGit2 class="proj-icon" />
            <span class="proj-name">{{ project.name }}</span>
            <span class="spacer" />
            <Button
              size="xs"
              variant="ghost"
              :disabled="!!checking[restModules(project)[0]?.path]"
              @click="checkProject(project)"
            >
              <CircleDot class="icon-xs" /> 检查改动
            </Button>
          </div>

          <!-- 分支 / 模块矩阵 -->
          <div class="branches">
            <div v-for="b in branchSummary(project)" :key="b.env" class="branch-row">
              <Badge variant="secondary" class="env-badge">
                <GitBranch class="badge-icon" /> {{ b.env }}
              </Badge>
              <span class="modules">
                <span v-for="m in b.modules" :key="m" class="mod-chip">{{ m }}</span>
              </span>
            </div>
          </div>

          <!-- rest 模块本地状态 -->
          <div v-if="restModules(project).length" class="rest-status">
            <div v-for="rest in restModules(project)" :key="rest.path" class="rest-row">
              <span class="rest-label mono">{{ rest.label }}</span>
              <span :class="['dirty', dirtyLabel(rest.path).tone]">{{ dirtyLabel(rest.path).text }}</span>
            </div>
          </div>

          <!-- 快捷入口 -->
          <div class="card-actions">
            <Button size="xs" variant="ghost" @click="emit('navigate', 'config', project.name)">
              <FileCog class="icon-xs" /> 配置
            </Button>
            <Button size="xs" variant="ghost" @click="emit('navigate', 'merge', project.name)">
              <GitMerge class="icon-xs" /> 合并
            </Button>
            <Button size="xs" variant="ghost" @click="emit('navigate', 'package', project.name)">
              <Package class="icon-xs" /> 打包
            </Button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.board-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--mat-content);
  overflow: hidden;
}
.board-head {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 10px 14px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
}
.title {
  font-size: var(--fs-callout);
  font-weight: 600;
  color: var(--fg);
}
.steps {
  display: flex;
  align-items: center;
  gap: 6px;
}
.step {
  font-size: var(--fs-caption);
  color: var(--fg-subtle);
}
.step-arrow {
  font-size: var(--fs-caption);
  color: var(--fg-subtle);
  opacity: 0.5;
}
.spacer {
  flex: 1;
}
.board-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 12px;
}
.cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 12px;
}
.proj-card {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
  border-radius: 8px;
  background: var(--mat-toolbar);
  box-shadow: var(--stroke-control);
}
.card-head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.proj-icon {
  width: 16px;
  height: 16px;
  color: var(--accent);
}
.proj-name {
  font-size: var(--fs-body);
  font-weight: 600;
  color: var(--fg);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.branches {
  display: flex;
  flex-direction: column;
  gap: 5px;
}
.branch-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.env-badge {
  gap: 3px;
}
.badge-icon {
  width: 11px;
  height: 11px;
}
.modules {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.mod-chip {
  font-size: var(--fs-caption);
  color: var(--fg-muted);
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--mat-elevated);
  box-shadow: var(--stroke-control);
}
.rest-status {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 6px 0;
  border-top: var(--hairline) solid var(--stroke-soft);
  border-bottom: var(--hairline) solid var(--stroke-soft);
}
.rest-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}
.rest-label {
  font-size: var(--fs-mono);
  color: var(--fg-muted);
}
.dirty {
  font-size: var(--fs-caption);
}
.dirty.clean {
  color: var(--success, #22c55e);
}
.dirty.dirty {
  color: var(--accent);
}
.dirty.unknown {
  color: var(--fg-subtle);
}
.dirty.error {
  color: var(--danger);
}
.card-actions {
  display: flex;
  gap: 6px;
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
</style>
