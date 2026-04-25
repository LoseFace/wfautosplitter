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

// function disableBrowserShortcuts() {
//   window.addEventListener("contextmenu", (e) => {
//     e.preventDefault()
//   })

//   window.addEventListener("keydown", (e) => {
//     const key = e.key.toLowerCase()

//     if (
//       key === "f5" ||
//       (e.ctrlKey && key === "r") ||
//       (e.ctrlKey && key === "p")
//     ) {
//       e.preventDefault()
//     }
//   })
// }

// disableBrowserShortcuts()

async function bootstrap() {
  const appWindow = getCurrentWindow()
  const windowLabel = appWindow.label

  if (windowLabel === 'overlay-window') {
    const app = createApp(OverlayWindow)
    app.use(i18n)
    app.mount("#app")
    return
  }

  await initSettings()

  let timeout: number | null = null
  watch(
    settings,
    (val) => {
      if (timeout) clearTimeout(timeout)
      timeout = window.setTimeout(() => {
        invoke("set_settings", { newSettings: val })
      }, 100)
    },
    { deep: true }
  )

  initTheme()

  await appWindow.setSize(
    new LogicalSize(
      Math.max(settings.window.x, 700),
      Math.max(settings.window.y, 250)
    )
  )

  appWindow.onResized(async () => {
    const size = await appWindow.innerSize()
    settings.window.x = size.width
    settings.window.y = size.height
  })

  const overlayWindow = await WebviewWindow.getByLabel('overlay-window')
  if (overlayWindow) {
    if (settings.overlay.show) {
      await overlayWindow.show()
    } else {
      await overlayWindow.hide()
    }
  }

  const app = createApp(App)
  app.use(i18n)
  app.mount("#app")
}

bootstrap()