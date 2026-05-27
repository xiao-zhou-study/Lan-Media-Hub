<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useShareStore } from '../stores/share'

const shareStore = useShareStore()
const activeNav = ref('home')
const toast = ref('')
const toastType = ref('success')
const hostname = ref('')
const serverUrl = ref('')
const serverRunning = ref(false)
const hasPassword = ref(false)
const showPassword = ref(false)
const passwordInput = ref('')
const showSettings = ref(false)
const browsingShare = ref('')
const browsingPath = ref('')
const browsingName = ref('')
const files = ref<any[]>([])
const loadingFiles = ref(false)
const viewMode = ref<'list' | 'grid'>('grid')
const showAddDialog = ref(false)
const newSharePath = ref('')
const newShareName = ref('')
const askingToDelete = ref('')
let invoke: any = null
let openDialog: any = null

onMounted(async () => {
  try { const c = await import('@tauri-apps/api/core'); invoke = c.invoke } catch { return }
  try { const d = await import('@tauri-apps/plugin-dialog'); openDialog = d.open } catch {}
  await shareStore.loadShares()
  try { const info = await invoke('get_server_info'); hostname.value = info.hostname; serverUrl.value = info.url; hasPassword.value = info.has_password; serverRunning.value = info.server_running } catch {}
})

function toastMsg(msg: string, t = 'success') { toast.value = msg; toastType.value = t; setTimeout(() => toast.value = '', 3000) }
async function copyText(t: string) { try { await navigator.clipboard.writeText(t); toastMsg('已复制') } catch { toastMsg('复制失败', 'error') } }
async function browseShare(id: string, name: string) { browsingShare.value = id; browsingPath.value = ''; browsingName.value = name; await loadFiles('') }
async function loadFiles(subPath: string) { loadingFiles.value = true; try { files.value = await invoke('browse_folder', { shareId: browsingShare.value, subPath: subPath || '' }); browsingPath.value = subPath } catch { toastMsg('加载失败', 'error') }; loadingFiles.value = false }
function navTo(path: string) { loadFiles(path) }
function goBack() { if (!browsingPath.value) { browsingShare.value = ''; files.value = [] } else { const p = browsingPath.value.split("/").filter(Boolean); p.pop(); loadFiles(p.join('/')) } }
async function pickFolder() { if (!openDialog) return; const s = await openDialog({ directory: true, multiple: false }); if (s) { newSharePath.value = s as string; if (!newShareName.value) { const p = (s as string).split(String.fromCharCode(92)).join("/").split("/"); newShareName.value = p[p.length - 1] || "" } } }
async function doAdd() { if (!newSharePath.value || !invoke) return; try { await invoke('add_share', { path: newSharePath.value, name: newShareName.value || null }); await shareStore.loadShares(); showAddDialog.value = false; newSharePath.value = ''; newShareName.value = ''; toastMsg('添加成功') } catch (e: any) { toastMsg('添加失败: ' + (e?.toString() || ''), 'error') } }
async function doDelete() { if (!askingToDelete.value || !invoke) return; try { await invoke('remove_share', { id: askingToDelete.value }); await shareStore.loadShares(); askingToDelete.value = ''; toastMsg('已删除') } catch { toastMsg('删除失败', 'error') } }
async function openUrl() { try { const { openUrl } = await import('@tauri-apps/plugin-opener'); await openUrl(serverUrl.value) } catch { window.open(serverUrl.value, '_blank') } }
async function savePw() { if (!invoke) return; try { await invoke('set_password', { password: passwordInput.value }); hasPassword.value = !!passwordInput.value; toastMsg('密码已保存') } catch { toastMsg('保存失败', 'error') } }
function fmtSize(b: number) { if (b < 1024) return b + ' B'; if (b < 1048576) return (b / 1024).toFixed(1) + ' KB'; if (b < 1073741824) return (b / 1048576).toFixed(1) + ' MB'; return (b / 1073741824).toFixed(1) + ' GB' }
function fileIcon(f: any) { if (f.is_dir) return 'folder'; const ext = (f.name.split('.').pop() || '').toLowerCase(); if ('mp4|mkv|avi|mov|webm|wmv|flv|mpg|mpeg|ts|mts|m2ts|vob|rm|rmvb|3gp|asf|divx|ogv|m4v'.includes(ext)) return 'video'; if ('mp3|flac|wav|aac|ogg|m4a'.includes(ext)) return 'audio'; if ('jpg|jpeg|png|gif|bmp|webp|svg|ico'.includes(ext)) return 'image'; return 'file' }
const stats = computed(() => ({ count: shareStore.shares.length, files: shareStore.shares.reduce((a, s) => a + s.file_count, 0), size: shareStore.shares.reduce((a, s) => a + s.total_size, 0) }))
</script>

<template>
<div class="flex h-screen bg-slate-50 text-slate-800 font-sans">
  <transition name="fade"><div v-if="toast" class="fixed top-5 left-1/2 -translate-x-1/2 z-50 px-5 py-2.5 rounded-xl text-sm font-medium shadow-xl" :class="toastType==='error'?'bg-red-500 text-white':'bg-slate-800 text-white'">{{ toast }}</div></transition>

  <aside class="w-56 bg-white border-r border-slate-200 flex flex-col shrink-0 shadow-sm z-10">
    <div class="px-4 py-4 border-b border-slate-100">
      <div class="flex items-center gap-2.5">
        <div class="w-8 h-8 rounded-lg bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center text-white shadow-md text-sm font-bold">L</div>
        <div><div class="text-sm font-bold">Lan Media Hub</div><div class="text-xs text-slate-400">Personal Cloud</div></div>
      </div>
    </div>
    <nav class="flex-1 py-3 px-2.5 space-y-1">
      <button @click="activeNav='home';browsingShare=''" :class="activeNav==='home'?'bg-blue-50 text-blue-600 shadow-sm':'text-slate-500 hover:bg-slate-50'" class="w-full flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-sm">📊 Overview</button>
      <button @click="activeNav='files';browsingShare=''" :class="activeNav==='files'?'bg-blue-50 text-blue-600 shadow-sm':'text-slate-500 hover:bg-slate-50'" class="w-full flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-sm">📁 Files</button>
      <button @click="showSettings=!showSettings" :class="showSettings?'bg-blue-50 text-blue-600 shadow-sm':'text-slate-500 hover:bg-slate-50'" class="w-full flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-sm">⚙ Settings</button>
    </nav>
    <div class="px-3 py-3 border-t border-slate-100"><div class="px-3 py-2 text-xs text-slate-400 bg-slate-50 rounded-lg flex items-center gap-2"><div class="w-2 h-2 rounded-full" :class="serverRunning?'bg-emerald-400':'bg-slate-300'"></div>{{ serverRunning?'Running':'Stopped' }}</div></div>
  </aside>

  <main class="flex-1 flex flex-col overflow-hidden">
    <header class="px-5 py-2.5 bg-white border-b border-slate-200 flex items-center justify-between shrink-0">
      <h2 class="text-sm font-semibold">{{ browsingShare ? browsingName : 'Dashboard' }}</h2>
      <div class="flex items-center gap-3 text-xs text-slate-500"><span>{{ hostname }}</span><a :href="serverUrl" target="_blank" class="text-blue-500 font-mono" @click.prevent="openUrl">{{ serverUrl }}</a></div>
    </header>

    <div class="flex-1 overflow-auto p-5">
      <div v-if="!browsingShare" class="max-w-3xl">
        <div class="flex items-center justify-between mb-4"><h3 class="text-sm font-semibold">Shared Folders</h3><button @click="showAddDialog=true" class="px-3 py-1.5 bg-blue-500 text-white text-xs rounded-lg hover:bg-blue-600">+ Add</button></div>
        <div v-if="showAddDialog" class="mb-3 bg-white rounded-xl border border-slate-200 p-4 space-y-2 shadow-sm"><h4 class="text-sm font-semibold">Add Share</h4><div class="flex gap-2"><input v-model="newSharePath" placeholder="Folder path" class="flex-1 px-2 py-1.5 bg-slate-50 border rounded-lg text-xs" /><button @click="pickFolder" class="px-3 py-1.5 bg-slate-200 text-xs rounded-lg hover:bg-slate-300">Browse</button></div><input v-model="newShareName" placeholder="Name (optional)" class="w-full px-2 py-1.5 bg-slate-50 border rounded-lg text-xs" /><div class="flex gap-2 justify-end"><button @click="showAddDialog=false" class="px-3 py-1.5 text-xs text-slate-500">Cancel</button><button @click="doAdd" :disabled="!newSharePath" class="px-4 py-1.5 bg-blue-500 text-white text-xs rounded-lg disabled:opacity-40">Confirm</button></div></div>
        <div v-if="shareStore.shares.length===0 && !showAddDialog" class="text-center py-16 text-slate-400"><div class="text-3xl mb-2">📂</div><p class="text-sm">No shared folders</p></div>
        <div v-else class="space-y-2"><div v-for="s in shareStore.shares" :key="s.id" @click="browseShare(s.id,s.name)" class="bg-white rounded-xl border border-slate-100 p-4 flex items-center gap-3 cursor-pointer hover:border-blue-200 hover:shadow-md transition-all group shadow-sm"><div class="w-10 h-10 rounded-lg bg-orange-50 flex items-center justify-center text-xl shrink-0">📁</div><div class="flex-1 min-w-0"><div class="text-sm font-semibold group-hover:text-blue-600">{{ s.name }}</div><div class="text-xs text-slate-400 truncate">{{ s.path }}</div><div class="text-xs text-slate-400">{{ s.file_count }} files · {{ fmtSize(s.total_size) }}</div></div><button @click.stop="askingToDelete=s.id" class="opacity-0 group-hover:opacity-100 px-2 py-1 text-xs text-red-500 hover:bg-red-50 rounded">Delete</button></div></div>
      </div>

      <div v-if="browsingShare" class="flex flex-col">
        <div class="flex items-center gap-3 mb-3"><button @click="goBack" class="px-3 py-1.5 text-xs bg-slate-200 rounded-lg hover:bg-slate-300">Back</button><div class="text-xs text-slate-500 overflow-x-auto flex items-center gap-1"><button @click="loadFiles('')" class="text-blue-500 shrink-0">{{ browsingName }}</button><template v-for="(p,i) in browsingPath.split('/').filter(Boolean)" :key="i"><span class="text-slate-300">/</span><button v-if="i<browsingPath.split('/').filter(Boolean).length-1" @click="navigateTo(browsingPath.split('/').filter(Boolean).slice(0,i+1).join('/'))" class="text-blue-500 shrink-0">{{ p }}</button><span v-else class="text-slate-600 shrink-0">{{ p }}</span></template></div></div>
        <div v-if="loadingFiles" class="text-center py-12 text-slate-400 text-sm">Loading...</div>
        <div v-else-if="files.length===0" class="text-center py-12 text-slate-400 text-sm">Empty</div>
        <div v-else class="bg-white rounded-xl border shadow-sm overflow-hidden"><div v-for="f in files" :key="f.path" class="flex items-center gap-3 px-4 py-2.5 border-b border-slate-50 last:border-0 hover:bg-slate-50 cursor-pointer text-sm"><span class="text-lg w-6 text-center">{{ f.is_dir?'📁':'📄' }}</span><span class="flex-1 truncate" :class="f.is_dir?'font-medium text-blue-600':''">{{ f.name }}</span><span class="text-xs text-slate-400">{{ f.is_dir?'':fmtSize(f.size) }}</span></div></div>
      </div>

      <div v-if="showSettings" class="max-w-md mt-4"><div class="bg-white rounded-xl border border-slate-100 p-5 shadow-sm space-y-3"><h3 class="text-sm font-semibold">Password</h3><p class="text-xs text-slate-400">Required for mobile access</p><div class="flex gap-2"><input :type="showPassword?'text':'password'" v-model="passwordInput" placeholder="Leave empty for no password" class="flex-1 px-2 py-1.5 bg-slate-50 border rounded-lg text-xs" /><button @click="showPassword=!showPassword" class="px-3 py-1.5 bg-slate-200 text-xs rounded-lg">{{ showPassword?'Hide':'Show' }}</button></div><div class="flex items-center gap-2"><button @click="savePw" class="px-4 py-1.5 bg-blue-500 text-white text-xs rounded-lg hover:bg-blue-600">Save</button><span v-if="hasPassword" class="text-xs text-emerald-500">Password set</span></div></div></div>
    </div>
  </main>

  <div v-if="askingToDelete" class="fixed inset-0 bg-black/30 flex items-center justify-center z-50" @click="askingToDelete=''"><div class="bg-white rounded-xl p-5 mx-4 shadow-xl w-72" @click.stop><h3 class="text-sm font-semibold mb-3">Delete share?</h3><p class="text-xs text-slate-500 mb-4">Files will not be deleted</p><div class="flex gap-2 justify-end"><button @click="askingToDelete=''" class="px-4 py-1.5 bg-slate-200 text-xs rounded-lg">Cancel</button><button @click="doDelete" class="px-4 py-1.5 bg-red-500 text-white text-xs rounded-lg">Delete</button></div></div></div>
</div>
</template>
<style>.fade-enter-active,.fade-leave-active{transition:opacity .2s}.fade-enter-from,.fade-leave-to{opacity:0}</style>