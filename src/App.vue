<script setup lang="ts">
import { ref, nextTick } from 'vue'
import { getCurrentWindow, LogicalPosition } from '@tauri-apps/api/window'

import MissionsView from './views/Races.vue'
import OverlayView from './views/Overlay.vue'
import SettingsView from './views/Settings.vue'
import TemplatesView from './views/TemplatesView.vue'

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
          <img class="avatar" src="./imgs/profile.png">
          <span class="text">{{playerNickname}}</span>
        </div>

        <nav class="nav">          
          <button @click="currentView = 'races'; racesKey++">
            <img src="./imgs/timer.png">
            <span class="text">{{ $t('races') }}</span>
          </button>

          <button @click="currentView = 'templates'">
            <img src="./imgs/template.png">
            <span class="text">{{ $t('templates') }}</span>
          </button>

          <button @click="currentView = 'overlay'">
            <img src="./imgs/overlay.png">
            <span class="text">{{ $t('overlay') }}</span>
          </button>

          <button @click="currentView = 'settings'">
            <img src="./imgs/settings.png">
            <span class="text">{{ $t('settings') }}</span>
          </button>

          <button class="update-button">
            <img src="./imgs/update.png">
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