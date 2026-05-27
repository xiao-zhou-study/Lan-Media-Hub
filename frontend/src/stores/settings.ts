import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useSettingsStore = defineStore('settings', () => {
  const port = ref(8080)
  const autoStart = ref(true)
  const serverRunning = ref(false)
  const serverUrl = ref('')

  async function loadSettings() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const settings = await invoke('get_settings') as any
      port.value = settings.port
      autoStart.value = settings.auto_start
    } catch { /* 非 Tauri 环境 */ }
  }

  async function getStatus() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const status = await invoke('get_status') as any
      serverRunning.value = status.server_running
      serverUrl.value = status.url
    } catch { /* 非 Tauri 环境 */ }
  }

  return { port, autoStart, serverRunning, serverUrl, loadSettings, getStatus }
})