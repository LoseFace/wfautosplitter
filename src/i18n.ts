import { createI18n } from 'vue-i18n'
import en from './locales/en.json'
import ru from './locales/ru.json'
import uk from './locales/uk.json'

const supportedLanguages = ['en', 'ru', 'uk']

let savedLocale = localStorage.getItem('locale')

let startLocale

if (!savedLocale || savedLocale === 'system') {
  const systemLang = navigator.language.slice(0, 2)

  startLocale = supportedLanguages.includes(systemLang)
    ? systemLang
    : 'en'
} else {
  startLocale = savedLocale
}

const i18n = createI18n({
  legacy: false,
  locale: startLocale,
  fallbackLocale: 'en',
  messages: { en, ru, uk }
})

export default i18n
