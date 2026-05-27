<script setup lang="ts">
import { ref, onMounted, watch, computed, nextTick } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import Plyr from 'plyr'
import 'plyr/dist/plyr.css'
import Hls from 'hls.js'

// 竖版视频适配
const plyrOverrideStyle = `
.plyr { max-width: 100% !important; max-height: 100% !important; }
.plyr__video-wrapper { max-height: 100% !important; }
.plyr video { max-width: 100% !important; max-height: 100% !important; object-fit: contain !important; }
`

const props = defineProps<{ shareId: string; path?: string[] }>()
const router = useRouter()
const route = useRoute()

const api = (window as any).$api
const files = ref<any[]>([])
const loading = ref(true)
const shareName = ref('')
const searchQuery = ref('')
const viewMode = ref<'grid' | 'list'>((localStorage.getItem('lm_view') as any) || 'grid')
const sortBy = ref<'name' | 'size' | 'modified'>((localStorage.getItem('lm_sort') as any) || 'name')
const sortOrder = ref<'asc' | 'desc'>((localStorage.getItem('lm_order') as any) || 'asc')

const previewUrl = ref('')
const previewType = ref('')
const previewName = ref('')
const previewDuration = ref(0)
const previewCurrentTime = ref(0)
const showPreview = ref(false)
const seekInput = ref('')
const isTranscoding = ref(false)
const currentFilePath = ref("")
const speedHint = ref('')
const speedSide = ref<'left' | 'right'>('right')
let plyr: Plyr | null = null
const videoRef = ref<HTMLVideoElement>()

const displayPath = computed(() => {
  const p = (route.params.path as any) || []
  const arr = Array.isArray(p) ? p : (typeof p === 'string' ? p.split('/') : [])
  return arr.filter(Boolean).join('/')
})
const breadcrumbs = computed(() => {
  const parts = displayPath.value.split('/').filter(Boolean)
  return parts.map((name, i) => ({ name, path: parts.slice(0, i + 1).join('/') }))
})

async function loadFiles() {
  loading.value = true
  try {
    const sub = displayPath.value
    const data = await api(`/api/browse/${props.shareId}${sub ? '/' + sub : ''}?sort=${sortBy.value}&order=${sortOrder.value}`)
    if (data.entries) files.value = data.entries
    else if (data.name) files.value = []
  } catch { files.value = [] }
  loading.value = false
}

onMounted(async () => {
  // 注入 Plyr 竖版视频适配样式
  const style = document.createElement('style'); style.textContent = plyrOverrideStyle; document.head.appendChild(style)
  try { const s = await api('/api/shares'); const f = s.find((x: any) => x.id === props.shareId); if (f) shareName.value = f.name } catch {}
  await loadFiles()
})

// watch with flush:post 确保 route 已更新
watch(() => [route.params.shareId, route.params.path], () => loadFiles(), { flush: 'post' })

watch(viewMode, (v) => { localStorage.setItem('lm_view', v) })
watch([sortBy, sortOrder], () => {
  localStorage.setItem('lm_sort', sortBy.value)
  localStorage.setItem('lm_order', sortOrder.value)
  loadFiles()
})

function navTo(subPath: string) { router.push({ name: 'browse', params: { shareId: props.shareId, path: subPath || '' } }) }
function goBack() {
  const parts = displayPath.value.split('/').filter(Boolean)
  if (!parts.length) { router.push({ name: 'home' }); return }
  parts.pop(); navTo(parts.join('/'))
}

async function handleClick(f: any) {
  if (f.is_dir) { navTo(f.path); return }
  const ext = (f.name.split('.').pop() || '').toLowerCase()
  // 浏览器原生支持的格式（白名单），其余全部 HLS 转码
  const nativeExts = ['mp4', 'm4v', 'webm', 'mkv', 'mov', 'ogv', 'ogg', 'mp3', 'wav', 'aac', 'm4a', 'flac']
  const needTranscode = f.media_type === 'video' && !nativeExts.includes(ext)
  previewType.value = f.media_type
  previewName.value = f.name
  previewDuration.value = 0
  previewCurrentTime.value = 0
  currentFilePath.value = f.path
  isTranscoding.value = needTranscode
  if (f.media_type === 'image') {
    previewUrl.value = withToken(`/api/stream/${props.shareId}/${f.path}`)
  } else {
    previewUrl.value = withToken(`/api/${needTranscode ? 'transcode' : 'stream'}/${props.shareId}/${f.path}`)
    // 获取视频时长
    if (f.media_type === 'video') {
      api(`/api/info/${props.shareId}/${f.path}`).then((info: any) => {
        if (info.duration) previewDuration.value = info.duration
      }).catch(() => {})
    }
  }
  showPreview.value = true
  // 初始化 Plyr 播放器
  if (f.media_type !== 'image') {
    await nextTick()
    if (videoRef.value) {
      plyr?.destroy()
      const v = videoRef.value
      if (needTranscode) {
        // HLS 流：hls.js 接管视频源
        if (Hls.isSupported()) {
          const hls = new Hls({ enableWorker: false })
          hls.loadSource(withToken(`/api/transcode/${props.shareId}/${f.path}`))
          hls.attachMedia(v)
          hls.on(Hls.Events.MANIFEST_PARSED, () => { v.play().catch(()=>{}) })
        } else if (v.canPlayType('application/vnd.apple.mpegurl')) {
          // Safari 原生 HLS
          v.src = withToken(`/api/transcode/${props.shareId}/${f.path}`)
        }
      }
      plyr = new Plyr(v, {
        controls: ['play', 'progress', 'current-time', 'duration', 'mute', 'fullscreen'],
        seekTime: 10,
        autoplay: !needTranscode,
      })
    }
  }
}

function doSeek() {
  const t = parseFloat(seekInput.value)
  if (isNaN(t) || t < 0) return
  const ext = (previewName.value.split('.').pop() || '').toLowerCase()
  const native = ['mp4', 'webm', 'ogg', 'mp3', 'wav', 'aac', 'm4a']
  previewCurrentTime.value = t
  if (!native.includes(ext)) {
    previewUrl.value = withToken(`/api/transcode/${props.shareId}/${displayPath.value ? displayPath.value + '/' : ''}${previewName.value}?start=${t}`)
  }
  // 原生格式用 video.currentTime
  const v = document.querySelector('video') as HTMLVideoElement
  if (v) v.currentTime = t
}

function fmtTime(s: number) {
  if (!s || s <= 0) return '--:--'
  const m = Math.floor(s / 60), sec = Math.floor(s % 60)
  return m + ':' + sec.toString().padStart(2, '0')
}

function closePreview() { showPreview.value = false; previewUrl.value = ''; plyr?.destroy(); plyr = null; stopLongPress() }

// === 长按倍速 / 滑动进度条 ===
let longPressTimer: any = null
let longPressInterval: any = null
let longPressing = false
let longPressStart = 0
let touchStartX = 0
let touchStartY = 0
let touchStartTime = 0

function onTouchStart(e: TouchEvent) {
  const t = e.touches[0]
  touchStartX = t.clientX
  touchStartY = t.clientY
  touchStartTime = Date.now()
  longPressStart = Date.now()
  const isRight = touchStartX > window.innerWidth / 2
  longPressing = true
  speedSide.value = isRight ? 'right' : 'left'
  longPressTimer = setTimeout(() => {
    const checkSpeed = () => {
        if (!longPressing) return
      const elapsed = (Date.now() - longPressStart) / 1000
      let s = isRight ? 1.5 : 0.75
      if (elapsed > 5) s = isRight ? 4.0 : 0.25
      else if (elapsed > 3) s = isRight ? 3.0 : 0.5
      else if (elapsed > 1.5) s = isRight ? 2.0 : 0.5
      const v = videoRef.value
      if (v) v.playbackRate = s
      speedHint.value = (isRight ? '▶▶ ' : '▶ ') + s.toFixed(1).replace(/\.0$/, '') + 'x'
      longPressInterval = setTimeout(checkSpeed, 400)
    }
    checkSpeed()
  }, 500)
}

function onTouchEnd(e: TouchEvent) {
  longPressing = false
  clearTimeout(longPressTimer)
  clearTimeout(longPressInterval)
  longPressInterval = null
  // 恢复原速（也重置 Plyr 内部状态）
  const v = videoRef.value
  if (v) {
    v.playbackRate = 1
    const pe = v.closest('.plyr') as any; if (pe && pe.plyr) pe.plyr.speed = 1
  }
  speedHint.value = ''
  // 保持显示 600ms 后淡出
  setTimeout(() => { speedHint.value = '' }, 600)

  const dx = e.changedTouches[0].clientX - touchStartX
  const dy = Math.abs(e.changedTouches[0].clientY - touchStartY)
  const dt = Date.now() - touchStartTime
  // 快速滑动 → seek
  if (Math.abs(dx) > 30 && dt < 500 && Math.abs(dx) > dy) {
    if (v && v.duration) {
      v.currentTime = Math.max(0, Math.min(v.duration, v.currentTime + (dx > 0 ? 10 : -10)))
    }
  }
}

function stopLongPress() {
  longPressing = false
  clearTimeout(longPressTimer)
  clearTimeout(longPressInterval)
  longPressInterval = null
  const v = videoRef.value; if (v) v.playbackRate = 1
  // 保持显示 600ms 后淡出
  setTimeout(() => { speedHint.value = '' }, 600)
}

function onTouchMove(e: TouchEvent) {
  if (longPressInterval) return
  // 侧边滑动 → 关闭播放器
  const edgeX = Math.abs(touchStartX - (touchStartX < window.innerWidth / 2 ? 0 : window.innerWidth))
  if (edgeX < 40 && e.touches[0].clientX > touchStartX + 60) {
    closePreview()
    return
  }
  if (edgeX < 40 && e.touches[0].clientX < touchStartX - 60) {
    closePreview()
    return
  }
  const dx = Math.abs(e.touches[0].clientX - touchStartX)
  const dy = Math.abs(e.touches[0].clientY - touchStartY)
  if (dx > 20 && dx > dy) e.preventDefault()
}

function fmtSize(b: number) { if (!b) return ''; if (b < 1024) return b + ' B'; if (b < 1048576) return (b / 1024).toFixed(1) + ' KB'; if (b < 1073741824) return (b / 1048576).toFixed(1) + ' MB'; return (b / 1073741824).toFixed(1) + ' GB' }

const token = ref((window as any).$token?.value || localStorage.getItem('lmtoken') || '')

function withToken(url: string) {
  return url + (url.includes('?') ? '&' : '?') + 'token=' + encodeURIComponent(token.value)
}

function thumbUrl(f: any) {
  if (f.is_dir) return ''
  const name = f.name.toLowerCase()
  // .bc! 后缀看前一个扩展名
  let ext = (name.split('.').pop() || '')
  if (ext === 'bc!') { const parts = name.split('.'); parts.pop(); ext = parts.pop() || '' }
  const videoExts = 'mp4|mkv|avi|mov|webm|wmv|flv|mpg|mpeg|ts|mts|m2ts|vob|rm|rmvb|3gp|asf|divx|ogv|m4v'
  const imageExts = 'jpg|jpeg|png|gif|webp|bmp'
  if (videoExts.includes(ext) || imageExts.includes(ext)) return withToken(`/api/thumbnail/${props.shareId}/${f.path}?size=200`)
  return ''
}

const filteredFiles = computed(() => {
  if (!searchQuery.value) return files.value
  const q = searchQuery.value.toLowerCase()
  return files.value.filter((f: any) => f.name.toLowerCase().includes(q))
})

</script>

<template>
  <div class="min-h-screen bg-[#f5f6fa] flex flex-col">
    <!-- Header -->
    <div class="bg-white border-b sticky top-0 z-20 shadow-sm">
      <div class="flex items-center gap-2 px-3 py-3">
        <button @click="goBack" class="text-blue-500 font-medium text-sm shrink-0">‹ 返回</button>
        <h1 class="text-sm font-semibold truncate flex-1">{{ shareName || '浏览' }}</h1>
      </div>
      <!-- Breadcrumb -->
      <div class="flex items-center gap-1 px-3 pb-1 text-[11px] text-gray-400 overflow-x-auto">
        <button @click="router.push({name:'home'})" class="text-blue-500 shrink-0">🏠</button>
        <button @click="navTo('')" class="text-blue-500 shrink-0">{{ shareName }}</button>
        <template v-for="(b, i) in breadcrumbs" :key="i">
          <span>/</span>
          <button v-if="i < breadcrumbs.length - 1" @click="navTo(b.path)" class="text-blue-500 shrink-0">{{ b.name }}</button>
          <span v-else class="text-gray-600 shrink-0">{{ b.name }}</span>
        </template>
      </div>
      <!-- Toolbar: search + sort + view -->
      <div class="flex items-center gap-2 px-3 pb-2.5">
        <div class="flex-1 relative">
          <input v-model="searchQuery" placeholder="搜索文件..." class="w-full pl-8 pr-3 py-1.5 bg-gray-100 border-0 rounded-full text-xs focus:outline-none focus:ring-2 focus:ring-blue-200" />
          <span class="absolute left-2.5 top-1/2 -translate-y-1/2 text-xs text-gray-400">🔍</span>
        </div>
        <select v-model="sortBy" class="text-[10px] bg-gray-100 border-0 rounded-full px-2 py-1.5 text-gray-600 outline-none">
          <option value="name">名称</option>
          <option value="size">大小</option>
          <option value="modified">时间</option>
        </select>
        <button @click="sortOrder = sortOrder === 'asc' ? 'desc' : 'asc'" class="text-[10px] bg-gray-100 rounded-full px-2 py-1.5 text-gray-600">{{ sortOrder === 'asc' ? '↑' : '↓' }}</button>
        <button @click="viewMode = viewMode === 'grid' ? 'list' : 'grid'" class="w-7 h-7 flex items-center justify-center rounded-full bg-gray-100 text-xs">{{ viewMode === 'grid' ? '☰' : '⊞' }}</button>
      </div>
    </div>

    <!-- Content -->
    <div class="flex-1 p-2">
      <div v-if="loading" class="flex items-center justify-center h-40 gap-2 text-gray-400 text-xs">
        <div class="w-4 h-4 border-2 border-gray-300 border-t-blue-500 rounded-full animate-spin"></div> 加载中
      </div>
      <div v-else-if="filteredFiles.length === 0" class="text-center py-20 text-gray-400">
        <div class="text-4xl mb-3">{{ searchQuery ? '🔍' : '📭' }}</div>
        <p class="text-sm">{{ searchQuery ? '无匹配结果' : '此目录为空' }}</p>
      </div>

      <!-- Grid -->
      <div v-else-if="viewMode === 'grid'" class="grid grid-cols-3 gap-2">
        <div v-for="f in filteredFiles" :key="f.path" @click="handleClick(f)" class="group active:scale-95 transition-transform duration-100">
          <div class="bg-white rounded-2xl overflow-hidden shadow-sm border border-gray-100 hover:shadow-md transition-shadow cursor-pointer">
            <!-- Thumbnail / Icon -->
            <div v-if="thumbUrl(f)" class="aspect-square bg-gray-100 flex items-center justify-center overflow-hidden">
              <img :src="thumbUrl(f)" loading="lazy" class="w-full h-full object-cover" @error="($event.target as HTMLImageElement).style.display='none'" />
            </div>
            <div v-else class="aspect-square flex items-center justify-center text-3xl" :class="{
              'bg-orange-50': f.is_dir,
              'bg-blue-50': !f.is_dir && !thumbUrl(f),
              'bg-gray-50': !f.is_dir,
            }">{{ f.is_dir ? '📁' : '📄' }}</div>
            <div class="p-2">
              <div class="text-[11px] font-medium text-gray-700 truncate">{{ f.name }}</div>
              <div class="text-[10px] text-gray-400 mt-0.5 flex justify-between">
                <span>{{ f.is_dir ? '-' : fmtSize(f.size) }}</span>
                <span v-if="f.modified" class="text-gray-300">{{ f.modified }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- List -->
      <div v-else class="space-y-0.5">
        <div v-for="f in filteredFiles" :key="f.path" @click="handleClick(f)" class="bg-white rounded-xl px-3 py-2.5 flex items-center gap-3 active:bg-gray-50 cursor-pointer text-sm border border-gray-50 shadow-sm">
          <div class="w-10 h-10 rounded-lg flex items-center justify-center shrink-0 overflow-hidden bg-gray-100">
            <img v-if="thumbUrl(f)" :src="thumbUrl(f)" loading="lazy" class="w-full h-full object-cover" @error="($event.target as HTMLImageElement).style.display='none'" />
            <span v-else class="text-lg">{{ f.is_dir ? '📁' : '📄' }}</span>
          </div>
          <div class="flex-1 min-w-0">
            <div class="font-medium truncate text-gray-700 text-xs">{{ f.name }}</div>
            <div class="text-[10px] text-gray-400">{{ f.modified || '' }}</div>
          </div>
          <div class="text-xs text-gray-400 shrink-0">{{ f.is_dir ? '' : fmtSize(f.size) }}</div>
          <div class="text-gray-300">›</div>
        </div>
      </div>
    </div>

    <!-- Preview -->
    <div v-if="showPreview" class="fixed inset-0 bg-black z-50 flex flex-col"
      @touchstart.passive="onTouchStart" @touchmove.prevent="onTouchMove" @touchend="onTouchEnd">
      <!-- Close button -->
      <button @click="closePreview" class="absolute top-3 right-3 z-20 w-9 h-9 rounded-full bg-white/20 hover:bg-white/30 text-white flex items-center justify-center text-lg backdrop-blur-sm">✕</button>
      <!-- Transcode loading -->

      <div class="absolute inset-0 flex items-center justify-center">
        <video v-if="previewType === 'video' || previewType === 'audio'"
          ref="videoRef" :src="isTranscoding ? undefined : previewUrl" :key="previewUrl"
          class="w-full h-full object-contain" playsinline />
        <!-- Speed hint -->
        <img v-else-if="previewType === 'image'" :src="previewUrl" class="w-full h-full object-contain" />
      </div>
    </div>
  </div>
</template>