<script setup lang="ts">
defineProps<{ size?: number; label?: string }>()
</script>

<template>
  <div class="spinner-wrap" :class="{ 'has-label': !!label }">
    <span
      class="spinner"
      :style="size ? { '--sz': `${size}px` } : undefined"
      role="status"
      aria-label="加载中"
    >
      <span v-for="i in 12" :key="i" :class="['tick', `t-${i}`]" />
    </span>
    <span v-if="label" class="spinner-label">{{ label }}</span>
  </div>
</template>

<style scoped>
.spinner-wrap {
  display: grid;
  place-items: center;
  gap: 8px;
  min-height: 64px;
  padding: 16px;
  color: var(--fg-muted);
}
.spinner {
  --sz: 18px;
  position: relative;
  width: var(--sz);
  height: var(--sz);
  display: inline-block;
  animation: rot 1s steps(12, end) infinite;
}
.tick {
  position: absolute;
  left: calc(50% - 0.5px);
  top: 0;
  width: 1.5px;
  height: 28%;
  border-radius: 999px;
  background: var(--fg-muted);
  transform-origin: 50% calc(var(--sz) / 2);
  opacity: 0.18;
}
/* 12 段以 30° 步进分布；亮度沿尾部递减形成"扫尾"效果 */
.t-1  { transform: rotate(0deg);   opacity: 1.00; }
.t-2  { transform: rotate(30deg);  opacity: 0.92; }
.t-3  { transform: rotate(60deg);  opacity: 0.80; }
.t-4  { transform: rotate(90deg);  opacity: 0.68; }
.t-5  { transform: rotate(120deg); opacity: 0.56; }
.t-6  { transform: rotate(150deg); opacity: 0.46; }
.t-7  { transform: rotate(180deg); opacity: 0.38; }
.t-8  { transform: rotate(210deg); opacity: 0.32; }
.t-9  { transform: rotate(240deg); opacity: 0.26; }
.t-10 { transform: rotate(270deg); opacity: 0.22; }
.t-11 { transform: rotate(300deg); opacity: 0.20; }
.t-12 { transform: rotate(330deg); opacity: 0.18; }

.spinner-label {
  font-size: var(--fs-callout);
  color: var(--fg-muted);
}

@keyframes rot {
  to {
    transform: rotate(360deg);
  }
}
@media (prefers-reduced-motion: reduce) {
  .spinner {
    animation-duration: 2.4s;
  }
}
</style>
