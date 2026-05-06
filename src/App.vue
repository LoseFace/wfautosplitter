<script setup lang="ts">
import { ref, nextTick } from 'vue'
import { getCurrentWindow, LogicalPosition } from '@tauri-apps/api/window'
import { getVersion } from '@tauri-apps/api/app'
import { openUrl } from '@tauri-apps/plugin-opener'
import changelogRaw from '../CHANGELOG.md?raw'

import MissionsView from './views/Races.vue'
import OverlayView from './views/Overlay.vue'
import SettingsView from './views/Settings.vue'
import TemplatesView from './views/TemplatesView.vue'
import imgTimer from './imgs/timer.png'
import imgTemplate from './imgs/template.png'
import imgOverlay from './imgs/overlay.png'
import imgSettings from './imgs/settings.png'
import imgUpdate from './imgs/update.png'

const showChangelog = ref(false)
const changelogText = ref('')
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

async function closeChangelog() {
  const currentVersion = await getVersion()
  settings.last_seen_version = currentVersion
  await invoke('set_settings', { newSettings: settings })
  showChangelog.value = false
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

  const currentVersion = await getVersion()
  const lastSeen = settings.last_seen_version

  if (lastSeen !== currentVersion) {
    changelogText.value = changelogRaw
    showChangelog.value = true
  }

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

  unlisten = await listen<string>('log-line', () => {})

})

onBeforeUnmount(() => {
  if (unlisten) {
    unlisten()
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
      <nav class="nav" ref="contentRef">          
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
    </aside>

    <div class="content">
      <div v-if="showChangelog" class="changelog-overlay">
        <div class="changelog-box">
          <label class="changelog-header">{{ $t('changelog_header') }}</label>
          <pre class="changelog-text">{{ changelogText }}</pre>
          <button class="changelog-close" @click="closeChangelog">
            {{ $t('close') }}
          </button>
        </div>
      </div>
      <component
        :is="views[currentView]"
        :key="currentView === 'races' ? racesKey : currentView"
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

.update{
  height: 100px;
  width: 100%;
}
.update-button{
  width: 100%;
  background-color: green;
}

.nav {
  display: flex;
  flex-direction: column;
  white-space: nowrap;
}

.nav button{
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

.nav img{
  width: 25px;
  height: 25px;
  flex-shrink: 0;
}

.text {
  flex-shrink: 0;
}

.changelog-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.5);
}

.changelog-box {
  background: var(--bg-color);
  box-shadow: 0 0 5px 2px rgba(0, 0, 0, 0.5);
  border-radius: 10px;
  width: 80vw;
  height: 80vh;
  display: flex;
  flex-direction: column;
  padding: 10px;
  gap: 12px;
}

.changelog-header{
  align-self: center;
  font-size: 25px;
}

.changelog-text {
  flex: 1;
  overflow-y: auto;
  white-space: pre-wrap;
  line-height: 1.3;
  margin: 0;
}

.changelog-close {
  align-self: center;
  height: 30px;
}
</style>