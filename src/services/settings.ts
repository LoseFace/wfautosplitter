import { reactive, watch } from "vue"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import type { AppSettings } from "../types/settings"

export const settings = reactive<AppSettings>({} as AppSettings)

let isInitialized = false
let isUpdatingFromRust = false
let isUpdatingFromVue = false
let updateTimeout: number | null = null

export async function initSettings() {
  const loaded = await invoke<AppSettings>("get_settings")

  Object.assign(settings, loaded)

  await updateGlobalShortcut()
  
  await listen("settings-updated", (event) => {
    isUpdatingFromRust = true
    Object.assign(settings, event.payload as AppSettings)
    isUpdatingFromRust = false
  })
  
  if (!isInitialized) {
    watch(
      () => [
        settings.overlay?.toggle_visibility_key,
        settings.overlay?.toggle_mode_key,
        settings.overlay?.run_reset_key,
      ],
      async (newKeys, oldKeys) => {
        if (isUpdatingFromRust || isUpdatingFromVue) return
        
        if (newKeys && oldKeys && newKeys[0] !== oldKeys[0] || newKeys[1] !== oldKeys[1]) {
          if (updateTimeout) clearTimeout(updateTimeout)
          updateTimeout = window.setTimeout(async () => {
            console.log(`Shortcuts changed, updating...`)
            await updateGlobalShortcut()
            updateTimeout = null
          }, 100)
        }
      },
      { deep: true }
    )
    isInitialized = true
  }
}

export async function updateGlobalShortcut() {
  if (!settings.overlay) return

  if (
    settings.overlay.toggle_visibility_key &&
    settings.overlay.toggle_visibility_key === settings.overlay.toggle_mode_key
  ) {
    console.error("Duplicate shortcuts are not allowed")
    return
  }
  
  isUpdatingFromVue = true 
  
  try {
    await invoke("register_shortcut_command", { 
      shortcutKey: settings.overlay.toggle_visibility_key, 
      shortcutType: "toggle_visibility" 
    })
    
    await invoke("register_shortcut_command", { 
      shortcutKey: settings.overlay.toggle_mode_key, 
      shortcutType: "toggle_mode" 
    })

    await invoke("register_shortcut_command", {
      shortcutKey: settings.overlay.run_reset_key,
      shortcutType: "run_reset"
    })
    
    console.log(`Global shortcuts updated`)
  } catch (error) {
    console.error("Failed to update global shortcuts:", error)
  } finally {
    isUpdatingFromVue = false
  }
}