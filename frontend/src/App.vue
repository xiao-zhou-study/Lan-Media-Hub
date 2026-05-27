<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useShareStore } from './stores/share'
import { useSettingsStore } from './stores/settings'
import TrayPopup from './components/TrayPopup.vue'

const shareStore = useShareStore()
const settingsStore = useSettingsStore()

onMounted(async () => {
  await shareStore.loadShares()
  await settingsStore.loadSettings()
})
</script>

<template>
  <div class="min-h-screen bg-gray-100">
    <TrayPopup />
  </div>
</template>