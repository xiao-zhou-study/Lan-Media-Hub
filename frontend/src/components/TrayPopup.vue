<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useShareStore } from '../stores/share'

const shareStore = useShareStore()
const toast = ref('')
const toastType = ref('success')
const hostname = ref('')
const serverUrl = ref('')
const serverRunning = ref(false)
const hasPassword = ref(false)
const showPassword = ref(false)
const passwordInput = ref('')
const browsingShare = ref('')
const browsingPath = ref('')
const browsingName = ref('')
const files = ref<any[]>([])
const loadingFiles = ref(false)
const showAddDialog = ref(false)
const newSharePath = ref('')
const newShareName = ref('')
const askingToDelete = ref('')
const qrCanvas = ref<HTMLCanvasElement>()
let invoke: any = null
let openDialog: any = null

onMounted(async () => {
  try { const c = await import('@tauri-apps/api/core'); invoke = c.invoke } catch { return }
  try { const d = await import('@tauri-apps/plugin-dialog'); openDialog = d.open } catch {}
  await shareStore.loadShares()
  try { const info = await invoke('get_server_info'); hostname.value = info.hostname; serverUrl.value = info.url; hasPassword.value = info.has_password; serverRunning.value = info.server_running; if (serverUrl.value) setTimeout(genQr,200) } catch {}
  try { const pw = await invoke("get_password"); if (pw) { passwordInput.value = pw; hasPassword.value = true } } catch {}
})

function genQr() { var cv=qrCanvas.value; if(!cv||!serverUrl.value) return; QRCode.toCanvas(cv, serverUrl.value, { width: 128, margin: 2 }, function(err) { if(err) console.error(err) }) }
function showToast(msg: string, t = 'success') { toast.value = msg; toastType.value = t; setTimeout(function() { toast.value = '' }, 3000) }
async function copyText(t: string) { try { await navigator.clipboard.writeText(t); showToast('已复制') } catch { showToast('复制失败', 'error') } }
async function browseShare(id: string, name: string) { browsingShare.value = id; browsingPath.value = ''; browsingName.value = name; await loadFiles('') }
async function loadFiles(subPath: string) { loadingFiles.value = true; try { files.value = await invoke('browse_folder', { shareId: browsingShare.value, subPath: subPath || '' }); browsingPath.value = subPath } catch(e) { showToast('加载失败: ' + (e||'').toString(), 'error') }; loadingFiles.value = false }
function navigateTo(path: string) { loadFiles(path) }
function goBack() { if (!browsingPath.value) { browsingShare.value = ''; files.value = [] } else { var parts = browsingPath.value.split('/').filter(Boolean); parts.pop(); loadFiles(parts.join('/')) } }
async function pickFolder() { if (!openDialog || !invoke) return; var selected = await openDialog({ directory: true, multiple: false }); if (selected) { var path = selected as string; var parts = path.split(String.fromCharCode(92)).join("/").split("/"); var name = parts[parts.length - 1] || ""; try { await invoke("add_share", { path: path, name: name }); await shareStore.loadShares(); showToast("已添加 " + name) } catch(e) { showToast("添加失败", "error") } } }
async function doAdd() { if (!newSharePath.value || !invoke) return; try { await invoke('add_share', { path: newSharePath.value, name: newShareName.value || null }); await shareStore.loadShares(); showAddDialog.value = false; newSharePath.value = ''; newShareName.value = ''; showToast('添加成功') } catch (e: any) { showToast('添加失败', 'error') } }
async function doDelete() { if (!askingToDelete.value || !invoke) return; try { await invoke('remove_share', { id: askingToDelete.value }); await shareStore.loadShares(); askingToDelete.value = ''; showToast('已删除') } catch { showToast('删除失败', 'error') } }
async function openUrl() { try { var m = await import('@tauri-apps/plugin-opener'); await m.openUrl(serverUrl.value) } catch { window.open(serverUrl.value, '_blank') } }
async function savePassword() { if (!invoke) return; try { await invoke('set_password', { password: passwordInput.value }); hasPassword.value = !!passwordInput.value; showToast('密码已保存') } catch { showToast('保存失败', 'error') } }
function fmtSize(b: number) { if (b < 1024) return b + ' B'; if (b < 1048576) return (b / 1024).toFixed(1) + ' KB'; if (b < 1073741824) return (b / 1048576).toFixed(1) + ' MB'; return (b / 1073741824).toFixed(1) + ' GB' }
const stats = computed(function() { return { count: shareStore.shares.length, files: shareStore.shares.reduce(function(a: number, s: any) { return a + s.file_count }, 0), size: shareStore.shares.reduce(function(a: number, s: any) { return a + s.total_size }, 0) } })

import QRCode from "qrcode";
</script>

<template>
<div class="h-screen bg-slate-50 text-slate-800 flex flex-col overflow-hidden">
  <transition name="fade"><div v-if="toast" class="fixed top-5 left-1/2 -translate-x-1/2 z-50 px-5 py-2.5 rounded-xl text-sm font-medium shadow-xl" :class="toastType==='error'?'bg-red-500 text-white':'bg-slate-800 text-white'">{{ toast }}</div></transition>

  <!-- 顶部栏 -->
  <header class="px-5 py-2.5 bg-white border-b border-slate-200 flex items-center justify-between shrink-0">
    <div class="flex items-center gap-3">
      <div class="w-7 h-7 rounded-lg bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center text-white text-xs font-bold shadow">L</div>
      <span class="text-sm font-bold">Lan Media Hub</span>
      <span class="text-xs text-slate-400">个人云</span>
    </div>
    <div class="flex items-center gap-4 text-xs text-slate-500">
      <span class="flex items-center gap-1"><span class="w-2 h-2 rounded-full" :class="serverRunning?'bg-emerald-400':'bg-slate-300'"></span>{{ serverRunning?'运行中':'未启动' }}</span>
      <span>{{ hostname }}</span>
      <a :href="serverUrl" target="_blank" class="text-blue-500 font-mono" @click.prevent="openUrl">{{ serverUrl }}</a>
    </div>
  </header>

  <!-- 连接信息卡片 -->
  <div class="px-5 py-3 bg-white border-b border-slate-100 flex items-center gap-4 shrink-0" style="min-height: 80px">
    <div class="w-16 h-16 bg-white rounded-lg flex items-center justify-center shrink-0 p-1 mr-4"><canvas ref="qrCanvas" style="max-width:64px;max-height:64px" width="128" height="128" class="w-full h-full"></canvas></div>
    <div class="space-y-1 text-xs">
      <div><span class="text-slate-400">地址：</span><code class="text-blue-600 bg-blue-50 px-1.5 py-0.5 rounded">{{ serverUrl }}</code><button @click="copyText(serverUrl)" class="ml-1 text-blue-500">复制</button></div>
      
      <p class="text-slate-400">手机同WiFi打开上方地址即可访问</p>
    </div>
    <!-- 密码设置 -->
    <div class="ml-auto flex items-center gap-2">
      <input :type="showPassword?'text':'password'" v-model="passwordInput" placeholder="访问密码" class="w-24 px-2 py-1 bg-slate-50 border rounded text-xs" />
      <button @click="showPassword=!showPassword" class="text-slate-400 text-xs">{{ showPassword?'隐藏':'显示' }}</button>
      <button @click="savePassword" class="px-2 py-1 bg-blue-500 text-white text-xs rounded hover:bg-blue-600">保存</button>
      
    </div>
  </div>

  <!-- 主内容区 -->
  <div class="flex-1 overflow-auto p-5">
    <!-- 文件浏览器 -->
    <div v-if="browsingShare">
      <div class="flex items-center gap-3 mb-3">
        <button @click="goBack" class="px-3 py-1.5 text-xs bg-slate-200 rounded-lg hover:bg-slate-300">← 返回</button>
        <div class="text-xs text-slate-500 overflow-x-auto flex items-center gap-1">
          <button @click="loadFiles('')" class="text-blue-500 shrink-0">{{ browsingName }}</button>
          <template v-for="(p,i) in browsingPath.split('/').filter(Boolean)" :key="i">
            <span class="text-slate-300">/</span>
            <button v-if="i<browsingPath.split('/').filter(Boolean).length-1" @click="navigateTo(browsingPath.split('/').filter(Boolean).slice(0,i+1).join('/'))" class="text-blue-500 shrink-0">{{ p }}</button>
            <span v-else class="text-slate-600 shrink-0">{{ p }}</span>
          </template>
        </div>
      </div>
      <div v-if="loadingFiles" class="text-center py-12 text-slate-400 text-sm">加载中...</div>
      <div v-else-if="files.length===0" class="text-center py-12 text-slate-400 text-sm">空目录</div>
      <div v-else class="bg-white rounded-xl border shadow-sm overflow-hidden">
        <div v-for="f in files" :key="f.path" @click="f.is_dir ? navigateTo(f.path) : null" class="flex items-center gap-3 px-4 py-2.5 border-b border-slate-50 last:border-0 hover:bg-slate-50 cursor-pointer text-sm">
          <span class="text-lg w-6 text-center">{{ f.is_dir?'📁':'📄' }}</span>
          <span class="flex-1 truncate" :class="f.is_dir?'font-medium text-blue-600':''">{{ f.name }}</span>
          <span class="text-xs text-slate-400">{{ f.is_dir?'':fmtSize(f.size) }}</span>
        </div>
      </div>
    </div>

    <!-- 共享列表 -->
    <div v-else>
      <div class="flex items-center justify-between mb-4">
        <div class="flex items-center gap-4">
          <h3 class="text-sm font-semibold">共享文件夹</h3>
          <span class="text-xs text-slate-400">{{ stats.count }} 个 | {{ stats.files.toLocaleString() }} 文件 | {{ fmtSize(stats.size) }}</span>
        </div>
        <button @click="pickFolder" class="px-3 py-1.5 bg-blue-500 text-white text-xs rounded-lg hover:bg-blue-600">+ 添加</button>
      </div>

      <div v-if="showAddDialog" class="mb-3 bg-white rounded-xl border border-slate-200 p-4 space-y-2 shadow-sm max-w-lg">
        <div class="flex gap-2"><input v-model="newSharePath" placeholder="文件夹路径" class="flex-1 px-2 py-1.5 bg-slate-50 border rounded-lg text-xs" /><button @click="pickFolder" class="px-3 py-1.5 bg-slate-200 text-xs rounded-lg hover:bg-slate-300">浏览</button></div>
        <input v-model="newShareName" placeholder="名称（可选）" class="w-full px-2 py-1.5 bg-slate-50 border rounded-lg text-xs" />
        <button @click="doAdd" :disabled="!newSharePath" class="w-full py-1.5 bg-blue-500 text-white text-xs rounded-lg disabled:opacity-40 hover:bg-blue-600">确认添加</button>
      </div>

      <div v-if="shareStore.shares.length===0 && !showAddDialog" class="text-center py-16 text-slate-400"><div class="text-3xl mb-2">📂</div><p class="text-sm">暂无共享文件夹</p><p class="text-xs mt-1">点击"+ 添加"开始</p></div>

      <div v-else class="space-y-2">
        <div v-for="s in shareStore.shares" :key="s.id" @click="browseShare(s.id,s.name)" class="bg-white rounded-xl border border-slate-100 p-4 flex items-center gap-3 cursor-pointer hover:border-blue-200 hover:shadow-md transition-all group shadow-sm">
          <div class="w-10 h-10 rounded-lg bg-orange-50 flex items-center justify-center text-xl shrink-0">📁</div>
          <div class="flex-1 min-w-0">
            <div class="text-sm font-semibold group-hover:text-blue-600">{{ s.name }}</div>
            <div class="text-xs text-slate-400 truncate">{{ s.path }}</div>
            <div class="text-xs text-slate-400">{{ s.file_count.toLocaleString() }} 文件 · {{ fmtSize(s.total_size) }}</div>
          </div>
          <button @click.stop="askingToDelete=s.id" class="opacity-0 group-hover:opacity-100 px-2 py-1 text-xs text-red-500 hover:bg-red-50 rounded">删除</button>
        </div>
      </div>
    </div>
  </div>

  <!-- 删除确认 -->
  <div v-if="askingToDelete" class="fixed inset-0 bg-black/30 flex items-center justify-center z-50" @click="askingToDelete=''"><div class="bg-white rounded-xl p-5 mx-4 shadow-xl w-72" @click.stop><h3 class="text-sm font-semibold mb-3">确认删除？</h3><p class="text-xs text-slate-500 mb-4">不会删除实际文件</p><div class="flex gap-2 justify-end"><button @click="askingToDelete=''" class="px-4 py-1.5 bg-slate-200 text-xs rounded-lg">取消</button><button @click="doDelete" class="px-4 py-1.5 bg-red-500 text-white text-xs rounded-lg">确认删除</button></div></div></div>
</div>
</template>
<style>.fade-enter-active,.fade-leave-active{transition:opacity .2s}.fade-enter-from,.fade-leave-to{opacity:0}</style>