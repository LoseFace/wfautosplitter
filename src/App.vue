<script setup lang="ts">
import { ref, nextTick } from 'vue'
import { getCurrentWindow, LogicalPosition } from '@tauri-apps/api/window'

import MissionsView from './views/Races.vue'
import OverlayView from './views/Overlay.vue'
import SettingsView from './views/Settings.vue'
import TemplatesView from './views/TemplatesView.vue'
import imgProfile from './imgs/profile.png'
import imgTimer from './imgs/timer.png'
import imgTemplate from './imgs/template.png'
import imgOverlay from './imgs/overlay.png'
import imgSettings from './imgs/settings.png'
import imgUpdate from './imgs/update.png'

import { getVersion } from '@tauri-apps/api/app'
import { openUrl } from '@tauri-apps/plugin-opener'

const hasUpdate = ref(false)
const downloadUrl = ref('')

async function checkForUpdates() {
  try {
    const current = await getVersion()
    const res = await fetch('https://api.github.com/repos/LoseFace/wfautosplitter/releases/latest')
    const data = await res.json()
    const latest = data.tag_name.replace('v', '')

    if (isNewerVersion(latest, current)) {
      hasUpdate.value = true
      const asset = data.assets.find((a: any) => a.name.endsWith('.exe'))
      if (asset) downloadUrl.value = asset.browser_download_url
    }
  } catch (e) {
    console.error('Update check failed:', e)
  }
}

function isNewerVersion(latest: string, current: string): boolean {
  const l = latest.split('.').map(Number)
  const c = current.split('.').map(Number)
  for (let i = 0; i < 3; i++) {
    if (l[i] > c[i]) return true
    if (l[i] < c[i]) return false
  }
  return false
}

async function handleUpdate() {
  if (downloadUrl.value) {
    await openUrl(downloadUrl.value)
  }
}

onMounted(() => {
  checkForUpdates()
})

const views = {
  races: MissionsView,
  templates: TemplatesView,
  overlay: OverlayView,
  settings: SettingsView,
} as const

type ViewName = keyof typeof views
const currentView = ref<ViewName>('races')
const racesKey = ref(0)

const menuRef = ref<HTMLElement | null>(null)
const contentRef = ref<HTMLElement | null>(null)

const CLOSED_WIDTH = 45

const openMenu = async () => {
  if (!menuRef.value || !contentRef.value) return

  await nextTick()

  const width = contentRef.value.scrollWidth
  menuRef.value.style.width = (width + 10) + 'px'
}

const closeMenu = () => {
  if (!menuRef.value) return
  menuRef.value.style.width = CLOSED_WIDTH + 'px'
}


import { onMounted, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useSettings } from './composables/useSettings'
const { settings } = useSettings()

let unlisten: UnlistenFn | null = null

onMounted(async () => {

  const logPath = settings.interface.path_log

  await invoke('start_log_reading', {
    path: logPath
  })

  const appWindow = getCurrentWindow()

  if (settings.window?.pos_x !== undefined && settings.window?.pos_y !== undefined) {
    try {
      await appWindow.setPosition(
        new LogicalPosition(settings.window.pos_x, settings.window.pos_y)
      )
    } catch {}
  }

  await appWindow.onMoved(async () => {
    try {
      const pos = await appWindow.outerPosition()
      settings.window.pos_x = pos.x
      settings.window.pos_y = pos.y
    } catch {}
  })

  unlisten = await listen<string>('log-line', (event) => {
    console.log('LOG FROM RUST:', event.payload)
  })

  const unlistenNickname = await listen<string>('player-nickname', (event) => {
    playerNickname.value = event.payload
    console.log('Player nickname updated:', event.payload)
  })

  unlisten = () => {
    if (unlisten) unlisten()
    unlistenNickname()
  }

})

onBeforeUnmount(() => {
  if (unlisten) {
    unlisten()
  }
})

const playerNickname = ref<string | null>(null)

onMounted(async () => {
  try {
    const result = await invoke<{
      lines: string[],
      nickname: string | null
    }>('read_log_snapshot', {
      path: settings.interface.path_log
    })

    playerNickname.value = result.nickname?.replace(/\s*\(.*\)$/, '') ?? null

  } catch (err) {
    console.error('Snapshot error:', err)
  }
})
</script>

<template>
  <div class="layout">
    
    <aside
      ref="menuRef"
      class="menu"
      @mouseenter="openMenu"
      @mouseleave="closeMenu"
    >
      <div ref="contentRef" class="menu-content">

        <div class="profile">
          <img class="avatar" :src="imgProfile">
          <span class="text">{{playerNickname}}</span>
        </div>

        <nav class="nav">          
          <button @click="currentView = 'races'; racesKey++">
            <img :src="imgTimer">
            <span class="text">{{ $t('races') }}</span>
          </button>

          <button @click="currentView = 'templates'">
            <img :src="imgTemplate">
            <span class="text">{{ $t('templates') }}</span>
          </button>

          <button @click="currentView = 'overlay'">
            <img :src="imgOverlay">
            <span class="text">{{ $t('overlay') }}</span>
          </button>

          <button @click="currentView = 'settings'">
            <img :src="imgSettings">
            <span class="text">{{ $t('settings') }}</span>
          </button>

          <button v-if="hasUpdate" class="update-button" @click="handleUpdate">
            <img :src="imgUpdate">
            <span class="text">{{ $t('update') }}</span>
          </button>
        </nav>
      </div>
    </aside>

    <div class="content">
      <component
        :is="views[currentView]"
        :key="currentView === 'races' ? racesKey : currentView"
        v-bind="currentView === 'races' ? { nickname: playerNickname } : {}"
      />
    </div>

  </div>
</template>

<style>
.layout {
  display: flex;
  height: 100vh;
  position: relative;
  background-color: var(--bg-color);
}

.menu {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 45px;
  min-width: 45px;
  overflow: hidden;
  transition: width 0.2s ease;
  box-shadow: 0 0 0 3px rgba(0,0,0,0.5);
  z-index: 1000;
  background: inherit;
}

.content {
  flex: 1;
  margin-left: 45px;
  overflow: auto;
}

.menu-content {
  display: flex;
  flex-direction: column;
  white-space: nowrap;
}

.update{
  height: 100px;
  width: 100%;
}
.update-button{
  width: 100%;
  background-color: green;
}

.profile {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px;
  height: 50px;
}

.nav {
  display: flex;
  flex-direction: column;
  padding-top: 20px;
}

.nav button,
.update-button{
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px;
  background: none;
  border: none;
  cursor: pointer;
  width: 100%;
  text-align: left;
}

.avatar, .nav img,
.update-button img{
  width: 25px;
  height: 25px;
  flex-shrink: 0;
}

.text {
  flex-shrink: 0;
}
</style>