import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface Share {
  id: string
  name: string
  path: string
  file_count: number
  total_size: number
  status: string
}

export const useShareStore = defineStore('share', () => {
  const shares = ref<Share[]>([])

  async function loadShares() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      shares.value = await invoke('get_shares')
    } catch { /* 非 Tauri 环境 */ }
  }

  return { shares, loadShares }
})