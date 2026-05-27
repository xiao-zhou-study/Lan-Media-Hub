<script setup lang="ts">
import { ref } from 'vue'

const token = ref(localStorage.getItem('lmtoken') || '')
const needsPw = ref(false)
const pwInput = ref('')
const pwError = ref(false)
const loading = ref(true)

// 通用 API 请求（带 JWT token）
async function api(url: string, opts?: RequestInit) {
  const headers: Record<string, string> = { ...(opts?.headers as any || {}) }
  if (token.value) headers['Authorization'] = 'Bearer ' + token.value
  const res = await fetch(url, { ...opts, headers })
  if (!res.ok) throw new Error(String(res.status))
  return res.json()
}

// 校验当前 token 是否有效
async function checkToken() {
  if (!token.value) { needsPw.value = true; loading.value = false; return }
  try {
    const r = await fetch('/api/shares', { headers: { 'Authorization': 'Bearer ' + token.value } })
    if (r.ok) { needsPw.value = false }
    else { needsPw.value = true; token.value = '' }
  } catch { needsPw.value = true; token.value = '' }
  loading.value = false
}

async function submitPw() {
  pwError.value = false
  try {
    const r = await fetch('/api/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ password: pwInput.value })
    })
    const data = await r.json()
    if (data.token) {
      token.value = data.token
      localStorage.setItem('lmtoken', data.token)
      needsPw.value = false
      pwInput.value = ''
    } else {
      pwError.value = true
    }
  } catch { pwError.value = true }
}

checkToken()

;(window as any).$api = api
;(window as any).$token = token
</script>

<template>
  <!-- Loading -->
  <div v-if="loading" class="min-h-screen flex items-center justify-center text-gray-400">
    <div class="w-5 h-5 border-2 border-gray-300 border-t-blue-500 rounded-full animate-spin"></div>
  </div>

  <!-- Password Gate -->
  <div v-else-if="needsPw" class="min-h-screen bg-gray-100 flex items-center justify-center p-4">
    <div class="bg-white rounded-xl shadow-lg p-6 w-full max-w-sm text-center">
      <div class="text-4xl mb-3">🔒</div>
      <h2 class="text-lg font-semibold mb-1">需要密码</h2>
      <p class="text-sm text-gray-500 mb-4">访问此共享需要输入密码</p>
      <input v-model="pwInput" type="password" placeholder="请输入密码"
        class="w-full px-3 py-2.5 border rounded-lg text-center text-sm mb-3 focus:outline-none focus:ring-2 focus:ring-blue-400"
        @keydown.enter="submitPw" />
      <p v-if="pwError" class="text-red-500 text-xs mb-2">密码错误</p>
      <button @click="submitPw" class="w-full py-2.5 bg-blue-500 text-white rounded-lg text-sm font-medium hover:bg-blue-600">确认</button>
    </div>
  </div>

  <!-- Main App -->
  <router-view v-slot="{ Component }" v-else>
    <keep-alive :max="5">
      <component :is="Component" />
    </keep-alive>
  </router-view>
</template>