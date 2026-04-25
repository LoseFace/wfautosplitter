import { watch } from "vue"
import { settings } from "../services/settings"
import { applyTheme, Theme } from "./themeManager"

export function initTheme() {
  watch(
    () => settings.interface.theme,
    (newTheme) => {
      applyTheme(newTheme as Theme)
    },
    { immediate: true }
  )
}