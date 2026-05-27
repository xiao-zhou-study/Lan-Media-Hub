<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'

const router = useRouter()
const shares = ref<any[]>([])
const loading = ref(true)
const api = (window as any).$api

onMounted(async () => { try { shares.value = await api('/api/shares') } catch {}; loading.value = false })
function fmtSize(b: number) { if (!b) return '-'; if (b < 1024) return b + ' B'; if (b < 1048576) return (b / 1024).toFixed(1) + ' KB'; if (b < 1073741824) return (b / 1048576).toFixed(1) + ' MB'; return (b / 1073741824).toFixed(1) + ' GB' }
</script>

<template>
  <div class="min-h-screen bg-[#f5f6fa]">
    <div class="bg-white border-b px-4 py-3.5 sticky top-0 z-20 shadow-sm">
      <h1 class="text-base font-bold flex items-center gap-2 text-gray-800">
        📡 Lan Media Hub
        <span class="text-[10px] font-normal text-gray-400 bg-gray-100 px-2 py-0.5 rounded-full">共享列表</span>
      </h1>
    </div>
    <div class="p-3">
      <div v-if="loading" class="flex justify-center py-20 text-gray-400 text-xs gap-2">
        <div class="w-4 h-4 border-2 border-gray-300 border-t-blue-500 rounded-full animate-spin"></div> 加载中
      </div>
      <div v-else-if="shares.length === 0" class="text-center py-20 text-gray-400">
        <div class="w-16 h-16 bg-gray-200 rounded-2xl flex items-center justify-center mx-auto mb-3 text-2xl">📂</div>
        <p class="text-sm font-medium text-gray-500">暂无共享文件夹</p>
        <p class="text-xs mt-1 text-gray-400">请在桌面应用中添加共享</p>
      </div>
      <div v-else class="space-y-2">
        <div v-for="s in shares" :key="s.id" @click="router.push({name:'browse',params:{shareId:s.id,path:''}})"
          class="bg-white rounded-2xl border border-gray-100 p-4 flex items-center gap-3.5 active:bg-gray-50 cursor-pointer shadow-sm hover:shadow-md transition-all">
          <div class="w-12 h-12 rounded-xl bg-gradient-to-br from-orange-100 to-orange-200 flex items-center justify-center text-2xl shrink-0">📁</div>
          <div class="flex-1 min-w-0">
            <div class="text-sm font-semibold text-gray-800">{{ s.name }}</div>
            <div class="text-xs text-gray-400 mt-0.5">{{ s.file_count.toLocaleString() }} 文件 · {{ fmtSize(s.total_size) }}</div>
          </div>
          <div class="text-gray-300 text-lg">›</div>
        </div>
      </div>
    </div>
  </div>
</template>