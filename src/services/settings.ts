import { reactive, watch } from "vue"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import type { AppSettings } from "../types/settings"

export const settings = reactive<AppSettings>({} as AppSettings)

let isInitialized    = false
let isUpdatingFromRust = false
let isUpdatingFromVue  = false
let updateTimeout: number | null = null

const SHORTCUT_KEYS = [
  ['toggle_visibility_key', 'toggle_visibility'],
  ['toggle_mode_key',       'toggle_mode'],
  ['run_reset_key',         'run_reset'],
] as const

export async function initSettings() {
  Object.assign(settings, await invoke<AppSettings>('get_settings'))

  await updateGlobalShortcut()

  await listen('settings-updated', (event) => {
    isUpdatingFromRust = true
    Object.assign(settings, event.payload as AppSettings)
    isUpdatingFromRust = false
  })

  if (isInitialized) return
  isInitialized = true

  watch(
    () => SHORTCUT_KEYS.map(([k]) => settings.overlay?.[k]),
    async (newKeys, oldKeys) => {
      if (isUpdatingFromRust || isUpdatingFromVue) return
      if (newKeys.some((v, i) => v !== oldKeys[i])) {
        if (updateTimeout) clearTimeout(updateTimeout)
        updateTimeout = window.setTimeout(async () => {
          await updateGlobalShortcut()
          updateTimeout = null
        }, 100)
      }
    },
  )
}

export async function updateGlobalShortcut() {
  if (!settings.overlay) return
  isUpdatingFromVue = true
  try {
    await Promise.all(
      SHORTCUT_KEYS.map(([settingKey, shortcutType]) =>
        invoke('register_shortcut_command', {
          shortcutKey: settings.overlay[settingKey],
          shortcutType,
        })
      )
    )
  } catch (e) {
    console.error('Failed to update global shortcuts:', e)
  } finally {
    isUpdatingFromVue = false
  }
}