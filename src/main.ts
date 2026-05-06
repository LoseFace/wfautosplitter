import { createApp, watch } from "vue"
import App from "./App.vue"
import OverlayWindow from "./views/OverlayWindow.vue"
import i18n from "./i18n"
import "./assets/global.css"
import { initTheme } from "./theme/initTheme"
import { initSettings, settings } from "./services/settings"
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window"
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { invoke } from "@tauri-apps/api/core"
import { listen, emit } from '@tauri-apps/api/event'
import { supportedLanguages, loadCustomLocales } from './i18n'
import {
  MODIFIER_CODES,
  eventToShortcutString,
  modifierOnlyShortcutFromKeyup,
} from './composables/useKeybindContext'

function resolveLocale(lang: string): string {
  const sys = supportedLanguages.includes(navigator.language.slice(0, 2))
    ? navigator.language.slice(0, 2)
    : 'en'
  return lang === 'system' ? sys
    : supportedLanguages.includes(lang) ? lang : 'en'
}

function applyLang(lang: string) {
  i18n.global.locale.value = resolveLocale(lang) as any
}

function listenLang() {
  listen('settings-updated', (e: any) => {
    const lang = e.payload?.interface?.language
    if (lang) applyLang(lang)
  })
}

function setupLocalHotkeyFallback() {
  let dirty = false

  function isOurShortcut(s: string) {
    const o = settings?.overlay
    return o && (s === o.toggle_visibility_key || s === o.toggle_mode_key || s === o.run_reset_key)
  }

  async function fireAction(shortcut: string) {
    const o = settings?.overlay
    if (!o) return
    if (shortcut === o.toggle_visibility_key) {
      o.show = !o.show
      await invoke('set_settings', { newSettings: { ...settings } })
    } else if (shortcut === o.toggle_mode_key) {
      await emit('toggle-overlay-mode')
    } else if (shortcut === o.run_reset_key) {
      await emit('run-reset')
      await emit('force-run-reset')
    }
  }

  window.addEventListener('keydown', async (e) => {
    if ((e.target as HTMLElement)?.closest?.('.keybind-capture')) return

    if (MODIFIER_CODES.has(e.code)) {
      if (!e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) dirty = false
      return
    }

    dirty = true
    const s = eventToShortcutString(e)
    if (s && isOurShortcut(s)) { e.preventDefault(); await fireAction(s) }
  }, true)

  window.addEventListener('keyup', async (e) => {
    if ((e.target as HTMLElement)?.closest?.('.keybind-capture')) return
    if (!MODIFIER_CODES.has(e.code)) return

    if (dirty) {
      if (!e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) dirty = false
      return
    }
    if (e.ctrlKey || e.altKey || e.shiftKey || e.metaKey) return

    dirty = false
    const s = modifierOnlyShortcutFromKeyup(e)
    if (s && isOurShortcut(s)) { e.preventDefault(); await fireAction(s) }
  }, true)
}

window.addEventListener('contextmenu', (e) => e.preventDefault())
window.addEventListener('keydown', (e) => {
  const k = e.key.toLowerCase()
  if (k === 'f5' || (e.ctrlKey && (k === 'r' || k === 'p'))) e.preventDefault()
})

async function bootstrap() {
  const windowLabel = getCurrentWindow().label

  if (windowLabel === 'overlay-window') {
    await initSettings()
    await loadCustomLocales(settings.interface?.custom_locales_dir)
    setupLocalHotkeyFallback()
    applyLang(settings.interface?.language ?? 'system')
    listenLang()
    createApp(OverlayWindow).use(i18n).mount('#app')
    return
  }

  await initSettings()
  await loadCustomLocales(settings.interface?.custom_locales_dir)
  setupLocalHotkeyFallback()
  applyLang(settings.interface.language ?? 'system')
  listenLang()

  let saveTimeout: number | null = null
  watch(settings, (val) => {
    if (saveTimeout) clearTimeout(saveTimeout)
    saveTimeout = window.setTimeout(() => invoke('set_settings', { newSettings: val }), 100)
  }, { deep: true })

  initTheme()

  const appWindow = getCurrentWindow()
  await appWindow.setSize(new LogicalSize(
    Math.max(settings.window.x, 700),
    Math.max(settings.window.y, 250),
  ))
  appWindow.onResized(async () => {
    const s = await appWindow.innerSize()
    settings.window.x = s.width
    settings.window.y = s.height
  })

  const overlayWindow = await WebviewWindow.getByLabel('overlay-window')
  if (overlayWindow) {
    await (settings.overlay.show ? overlayWindow.show() : overlayWindow.hide())
  }

  createApp(App).use(i18n).mount('#app')
}

bootstrap()