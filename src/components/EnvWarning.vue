<script setup lang="ts">
import { Terminal } from 'lucide-vue-next'

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'

defineProps<{ message: string }>()
const emit = defineEmits<{ retry: [] }>()
</script>

<template>
  <Alert variant="destructive" class="env-alert">
    <Terminal class="h-4 w-4" />
    <AlertTitle>未检测到可用的 svn 命令</AlertTitle>
    <AlertDescription>
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
      <Button size="sm" variant="outline" @click="emit('retry')">重新检测</Button>
    </div>
    </AlertDescription>
  </Alert>
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
