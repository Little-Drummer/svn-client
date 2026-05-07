// Monaco Editor 在 Vite 下需要把 worker 显式注入 MonacoEnvironment，否则会去 CDN 找 worker 文件失败。
// 仅做 diff 视图，editor + json 不需要的语言 worker 全部走默认 worker。

import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker'

let installed = false
export function ensureMonacoEnv() {
  if (installed) return
  installed = true
  ;(self as unknown as { MonacoEnvironment: { getWorker: () => Worker } }).MonacoEnvironment = {
    getWorker() {
      return new EditorWorker()
    },
  }
}
