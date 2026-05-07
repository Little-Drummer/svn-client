import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue()],
  server: {
    port: 5173,
    strictPort: true,
  },
  optimizeDeps: {
    include: ['monaco-editor/esm/vs/editor/editor.api'],
  },
  build: {
    target: 'es2020',
  },
})
