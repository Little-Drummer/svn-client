<script setup lang="ts">
import { FolderOpen } from 'lucide-vue-next'

defineProps<{ description: string }>()
</script>

<template>
  <div class="empty-state">
    <FolderOpen class="empty-icon" />
    <span>{{ description }}</span>
  </div>
</template>

<style scoped>
.empty-state {
  display: grid;
  place-items: center;
  gap: 7px;
  padding: 22px 14px;
  color: var(--text-muted);
  font-size: 12px;
  text-align: center;
}
.empty-icon {
  width: 22px;
  height: 22px;
  opacity: 0.52;
}
</style>
