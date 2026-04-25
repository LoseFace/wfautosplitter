<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSettings } from '../composables/useSettings'
import { applyTheme } from '../theme/themeManager'
import { open } from '@tauri-apps/plugin-dialog'
import { emit } from '@tauri-apps/api/event'

const { settings } = useSettings()

/* ================= LANGUAGE ================= */

const supportedLanguages = ['en', 'ru', 'uk'] as const
const { locale } = useI18n()

const selectedLanguage = ref<string>(settings.interface.language ?? 'system')

watch(selectedLanguage, async (newLang) => {
  settings.interface.language = newLang

  let resolvedLocale: string

  if (newLang === 'system') {
    const systemLang = navigator.language.slice(0, 2)
    resolvedLocale = supportedLanguages.includes(systemLang as any) ? systemLang : 'en'
  } else {
    resolvedLocale = newLang
  }

  locale.value = resolvedLocale
  await emit('language-changed', resolvedLocale)
})

onMounted(() => {
  const savedLang = settings.interface.language ?? 'system'

  if (savedLang === 'system') {
    const systemLang = navigator.language.slice(0, 2)

    locale.value = supportedLanguages.includes(systemLang as any)
      ? systemLang
      : 'en'
  } else {
    locale.value = savedLang
  }
})

/* ================= THEME ================= */

type Theme = 'light' | 'dark' | 'system'

const theme = ref<Theme>(settings.interface.theme as Theme ?? 'system')

watch(theme, (newVal) => {
  settings.interface.theme = newVal
  applyTheme(newVal)
})

onMounted(() => {
  applyTheme(theme.value)
})

/* ================= LOG PATH ================= */

const logPath = ref<string>(
  settings.interface.path_log ?? '%LocalAppData%\\Warframe'
)

watch(logPath, (val) => {
  settings.interface.path_log = val
})

const browseFolder = async () => {
  const selected = await open({
    directory: true,
    multiple: false,
  })

  if (selected && typeof selected === 'string') {
    logPath.value = selected
  }
}
</script>

<template>
  <div class="settings">
    <div class="sele">{{ $t('language') }}
      <select id="sele" v-model="selectedLanguage">
        <option value="system">{{ $t('language_system') }}</option>
        <option value="en">English</option>
        <option value="ru">Русский</option>
        <option value="uk">Українська</option>
      </select>
    </div>

    <div class="theme">{{ $t('theme') }}
      <select id="theme" v-model="theme">
        <option value="system">{{ $t('theme_default') }}</option>
        <option value="dark">{{ $t('theme_dark') }}</option>
        <option value="light">{{ $t('theme_light') }}</option>
      </select>
    </div>

    <div class="log">
      <div class="log-path-and-name">
        <div class="pathLog">{{ $t('path_log') }}</div>
        <div class="path-log-name">{{ logPath }}</div>
      </div>
      <button class="path-browse" @click="browseFolder">
        {{ $t('browse') }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.settings > div{
  padding: 10px;
}
.settings > div:hover{
  background-color: var(--stngs-bg-color);
}
.path-browse{
  margin-left: 10px;
}
.settings{
  display: flex;
  flex-direction: column;
  width: 100%;
}
.log{
  display: flex;
  flex-direction: row;
  width: 100%;
  min-width: 0;
}
.log-path-and-name {
  min-width: 0;
  overflow: hidden;
}
.path-log-name{
  font-size: 15px;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 400px;
  direction: rtl;
  unicode-bidi: plaintext;
}
</style>