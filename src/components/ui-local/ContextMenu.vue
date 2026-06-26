<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import type { Component } from 'vue'

export interface ContextMenuItem {
  key: string
  label?: string
  icon?: Component
  danger?: boolean
  disabled?: boolean
  separator?: boolean
}

const props = defineProps<{
  open: boolean
  x: number
  y: number
  items: ContextMenuItem[]
}>()

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
  (e: 'select', key: string): void
}>()

const menuRef = ref<HTMLElement | null>(null)

// 实际渲染坐标：打开后按菜单尺寸与视口边界修正，避免底部/右侧被裁切点不到
const left = ref(0)
const top = ref(0)
const position = computed(() => ({ left: `${left.value}px`, top: `${top.value}px` }))

const VIEWPORT_MARGIN = 8

async function reposition() {
  // 先用锚点摆放，再在下一帧拿到真实尺寸做钳制
  left.value = props.x
  top.value = props.y
  await nextTick()
  const el = menuRef.value
  if (!el) return
  const { width, height } = el.getBoundingClientRect()
  const maxLeft = window.innerWidth - width - VIEWPORT_MARGIN
  const maxTop = window.innerHeight - height - VIEWPORT_MARGIN
  left.value = Math.max(VIEWPORT_MARGIN, Math.min(props.x, maxLeft))
  top.value = Math.max(VIEWPORT_MARGIN, Math.min(props.y, maxTop))
}

function close() {
  emit('update:open', false)
}

function onSelect(item: ContextMenuItem) {
  if (item.disabled || item.separator) {
    return
  }
  emit('select', item.key)
  close()
}

// 打开时挂全局监听：点击别处、滚动、Esc、右键别处都关闭
function onPointerDown(e: MouseEvent) {
  if (menuRef.value && !menuRef.value.contains(e.target as Node)) {
    close()
  }
}
function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    close()
  }
}
function onScroll() {
  close()
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      reposition()
      // 捕获阶段监听，避免与行点击冲突
      document.addEventListener('pointerdown', onPointerDown, true)
      document.addEventListener('keydown', onKeydown, true)
      window.addEventListener('scroll', onScroll, true)
      window.addEventListener('resize', onScroll, true)
    } else {
      teardown()
    }
  },
)

function teardown() {
  document.removeEventListener('pointerdown', onPointerDown, true)
  document.removeEventListener('keydown', onKeydown, true)
  window.removeEventListener('scroll', onScroll, true)
  window.removeEventListener('resize', onScroll, true)
}

onBeforeUnmount(teardown)
</script>

<template>
  <Teleport to="body">
    <Transition name="ctx-fade">
      <div
        v-if="open"
        ref="menuRef"
        class="context-menu"
        :style="position"
        @contextmenu.prevent
      >
        <template v-for="item in items" :key="item.key">
          <div v-if="item.separator" class="ctx-separator" />
          <button
            v-else
            type="button"
            class="ctx-item"
            :class="{ 'ctx-danger': item.danger, 'ctx-disabled': item.disabled }"
            :disabled="item.disabled"
            @click="onSelect(item)"
          >
            <component :is="item.icon" v-if="item.icon" class="ctx-icon" />
            <span class="ctx-label">{{ item.label }}</span>
          </button>
        </template>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.context-menu {
  position: fixed;
  z-index: 1000;
  min-width: 168px;
  padding: 5px;
  border-radius: 10px;
  background: var(--mat-elevated, rgba(245, 245, 245, 0.92));
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  border: var(--hairline, 0.5px) solid var(--stroke, rgba(0, 0, 0, 0.12));
  box-shadow:
    0 0 0 0.5px rgba(0, 0, 0, 0.06),
    0 8px 28px rgba(0, 0, 0, 0.22);
  user-select: none;
}
.ctx-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 5px 9px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--fg, #1a1a1a);
  font-size: var(--fs-callout, 13px);
  text-align: left;
  cursor: default;
}
.ctx-item:hover:not(.ctx-disabled) {
  background: var(--accent, #0a84ff);
  color: #fff;
}
.ctx-item:hover:not(.ctx-disabled) .ctx-icon {
  color: #fff;
}
.ctx-danger {
  color: var(--danger, #e5484d);
}
.ctx-danger:hover:not(.ctx-disabled) {
  background: var(--danger, #e5484d);
  color: #fff;
}
.ctx-disabled {
  opacity: 0.4;
  cursor: default;
}
.ctx-icon {
  width: 14px;
  height: 14px;
  flex: none;
  color: var(--fg-muted, #888);
}
.ctx-label {
  flex: 1;
  white-space: nowrap;
}
.ctx-separator {
  height: 0.5px;
  margin: 4px 6px;
  background: var(--stroke-soft, rgba(0, 0, 0, 0.08));
}

.ctx-fade-enter-active {
  transition: opacity 90ms ease-out, transform 90ms cubic-bezier(0.32, 0.72, 0, 1);
}
.ctx-fade-enter-from {
  opacity: 0;
  transform: scale(0.96);
}
@media (prefers-reduced-motion: reduce) {
  .ctx-fade-enter-active {
    transition: none;
  }
}
</style>
