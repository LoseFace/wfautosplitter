import { createI18n } from 'vue-i18n'

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

export default i18n