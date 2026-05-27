<template>
  <div class="spinner-wrap">
    <span class="spinner" />
  </div>
</template>

<style scoped>
.spinner-wrap {
  display: grid;
  min-height: 56px;
  place-items: center;
  color: var(--text-muted);
}
.spinner {
  width: 18px;
  height: 18px;
  border: 2px solid color-mix(in srgb, var(--text-muted) 28%, transparent);
  border-top-color: var(--accent);
  border-radius: 999px;
  animation: spin 700ms linear infinite;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
