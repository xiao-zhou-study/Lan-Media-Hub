import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { codeInspectorPlugin } from 'code-inspector-plugin'

export default defineConfig({
  plugins: [
    vue(),
    codeInspectorPlugin({ bundler: 'vite' }),
  ],
  server: {
    port: 8242,
    proxy: {
      '/api': 'http://127.0.0.1:8241',
    }
  },
  build: { outDir: 'dist', assetsDir: 'assets' }
})