import { createI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'

const modules = import.meta.glob('./locales/*.json', { eager: true })

const messages: Record<string, any> = {}
const supportedLanguages: string[] = []

for (const path in modules) {
  const lang = path.match(/\/([^/]+)\.json$/)?.[1]
  if (lang) {
    messages[lang] = (modules[path] as any).default
    supportedLanguages.push(lang)
  }
}

export { supportedLanguages }

let savedLocale = localStorage.getItem('locale')
let startLocale: string

if (!savedLocale || savedLocale === 'system') {
  const systemLang = navigator.language.slice(0, 2)
  startLocale = supportedLanguages.includes(systemLang) ? systemLang : 'en'
} else {
  startLocale = supportedLanguages.includes(savedLocale) ? savedLocale : 'en'
}

const i18n = createI18n({
  legacy: false,
  locale: startLocale,
  fallbackLocale: 'en',
  messages,
})

export async function loadCustomLocales(dir: string | null | undefined) {
  if (!dir) return
  try {
    const locales = await invoke<[string, string][]>('read_custom_locales', { dir })
    for (const [langCode, jsonContent] of locales) {
      const parsed = JSON.parse(jsonContent)
      i18n.global.setLocaleMessage(langCode, parsed)
      if (!supportedLanguages.includes(langCode)) {
        supportedLanguages.push(langCode)
      }
    }
  } catch (e) {
    console.error('Error loading custom locales:', e)
  }
}

export default i18n