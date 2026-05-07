<script setup lang="ts">
import { NAlert, NButton } from 'naive-ui'

defineProps<{ message: string }>()
const emit = defineEmits<{ retry: [] }>()
</script>

<template>
  <n-alert type="error" closable class="env-alert" title="未检测到可用的 svn 命令">
    <div>{{ message }}</div>
    <div class="hint">
      请先安装 SVN 命令行工具：
      <ul>
        <li>macOS: <code>brew install subversion</code></li>
        <li>Windows: 安装 TortoiseSVN（勾选 command line tools）或 SlikSVN</li>
        <li>Linux: 使用包管理器安装 subversion</li>
      </ul>
      安装后可重启应用，或通过设置指定 svn 路径。
    </div>
    <div class="footer">
      <n-button size="small" @click="emit('retry')">重新检测</n-button>
    </div>
  </n-alert>
</template>

<style scoped>
.env-alert {
  margin: 8px 12px 0;
}
.hint {
  margin-top: 6px;
  font-size: 12px;
  opacity: 0.85;
}
.footer {
  margin-top: 8px;
}
</style>
