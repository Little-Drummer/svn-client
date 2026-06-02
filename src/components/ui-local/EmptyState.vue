<script setup lang="ts">
import type { Component } from 'vue'
import { Inbox } from 'lucide-vue-next'

defineProps<{
  title?: string
  description: string
  icon?: Component
}>()
</script>

<template>
  <div class="empty-state">
    <div class="empty-icon-wrap">
      <component :is="icon ?? Inbox" class="empty-icon" />
    </div>
    <div class="empty-text">
      <p v-if="title" class="empty-title">{{ title }}</p>
      <p class="empty-desc">{{ description }}</p>
    </div>
  </div>
</template>

<style scoped>
.empty-state {
  display: grid;
  place-items: center;
  gap: 12px;
  padding: 36px 20px;
  color: var(--fg-muted);
  text-align: center;
  animation: fade-up 240ms cubic-bezier(0.32, 0.72, 0, 1) both;
}
.empty-icon-wrap {
  display: grid;
  place-items: center;
  width: 44px;
  height: 44px;
  border-radius: 50%;
  background: color-mix(in srgb, var(--fg) 5%, transparent);
  box-shadow: inset 0 0 0 0.5px var(--stroke-soft);
}
.empty-icon {
  width: 20px;
  height: 20px;
  color: var(--fg-subtle);
  stroke-width: 1.6;
}
.empty-text {
  display: grid;
  gap: 4px;
  max-width: 280px;
}
.empty-title {
  margin: 0;
  font-size: var(--fs-body);
  font-weight: 600;
  color: var(--fg-strong);
  letter-spacing: -0.005em;
}
.empty-desc {
  margin: 0;
  font-size: var(--fs-callout);
  color: var(--fg-muted);
  line-height: 1.45;
}

@keyframes fade-up {
  from {
    opacity: 0;
    transform: translateY(4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
@media (prefers-reduced-motion: reduce) {
  .empty-state {
    animation: none;
  }
}
</style>
