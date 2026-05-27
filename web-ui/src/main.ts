import { createApp } from 'vue'
import { createRouter, createWebHashHistory } from 'vue-router'
import App from './App.vue'
import HomeView from './views/HomeView.vue'
import BrowseView from './views/BrowseView.vue'
import './assets/main.css'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', name: 'home', component: HomeView },
    { path: '/browse/:shareId/:path(.*)*', name: 'browse', component: BrowseView, props: true },
  ]
})

createApp(App).use(router).mount('#app')