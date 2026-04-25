<script setup lang="ts">
import { onMounted, onUnmounted, watch, ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'

import RaceStatistic from './components/RaceStatistic.vue'

const { t } = useI18n()
const STORAGE_KEY_RACES_SEARCH = 'races_search_text'
const STORAGE_KEY_RACES_SORT = 'races_sort_index'

const props = defineProps<{ nickname?: string | null }>()

let unlistenRunSaved: UnlistenFn | null = null

interface TemplateSummary {
  template_id: string
  template_name: string
  best_time: number
  best_run_date: number
  runs_count: number
  abort_count: number
}

const summaries = ref<TemplateSummary[]>([])
const loading   = ref(false)
const error     = ref<string | null>(null)

const searchRaceText = ref(localStorage.getItem(STORAGE_KEY_RACES_SEARCH) || '')
const sortIndex = ref(parseInt(localStorage.getItem(STORAGE_KEY_RACES_SORT) || '0'))

const selectedSummary = ref<TemplateSummary | null>(null)

function openStatistic(s: TemplateSummary) {
  selectedSummary.value = s
}

function closeStatistic() {
  selectedSummary.value = null
}

const SORT_MODES = computed(() => [
  t('sort_name') + ' ⇩',
  t('sort_name') + ' ⇧',
  t('sort_date') + ' ⇩',
  t('sort_date') + ' ⇧',
])

async function loadSummaries() {
  const nickname = props.nickname
  if (!nickname) {
    summaries.value = []
    return
  }

  loading.value = true
  error.value   = null
  try {
    summaries.value = await invoke<TemplateSummary[]>('get_template_summaries', { nickname })
  } catch (e: any) {
    error.value = e?.toString() ?? 'error'
  } finally {
    loading.value = false
  }
}

onMounted(async () => {
  await loadSummaries()
  unlistenRunSaved = await listen('run-saved', () => {
    loadSummaries()
  })
})
onUnmounted(() => {
  if (unlistenRunSaved) unlistenRunSaved()
})

watch(() => props.nickname, (val) => {
  if (val) loadSummaries()
  else summaries.value = []
})

watch(searchRaceText, (newVal) => {
  localStorage.setItem(STORAGE_KEY_RACES_SEARCH, newVal)
})

watch(sortIndex, (newVal) => {
  localStorage.setItem(STORAGE_KEY_RACES_SORT, newVal.toString())
})

function formatTime(sec: number): string {
  const totalMs  = Math.round(sec * 1000)
  const ms       = totalMs % 1000
  const totalSec = Math.floor(totalMs / 1000)
  const s        = totalSec % 60
  const totalMin = Math.floor(totalSec / 60)
  const m        = totalMin % 60
  const h        = Math.floor(totalMin / 60)

  const parts: string[] = []
  if (h > 0) parts.push(`${h}h`)
  if (m > 0) parts.push(`${m}m`)
  if (s > 0 || parts.length === 0) parts.push(`${s}s`)
  if (ms > 0) parts.push(`${ms}ms`)

  return parts.join(' ')
}

function formatDate(ts: number): string {
  const d  = new Date(ts * 1000)
  const dd = String(d.getDate()).padStart(2, '0')
  const mm = String(d.getMonth() + 1).padStart(2, '0')
  const yy = String(d.getFullYear()).slice(2)
  return `${dd}.${mm}.${yy}`
}

const filtered = computed(() => {
  const q = searchRaceText.value.trim().toLowerCase()

  let list = summaries.value.filter(s => {
    if (!q) return true
    if (s.template_name.toLowerCase().includes(q)) return true
    if (formatTime(s.best_time).toLowerCase().includes(q)) return true
    if (formatDate(s.best_run_date).includes(q)) return true
    return false
  })

  switch (sortIndex.value) {
    case 0: list = [...list].sort((a, b) => a.template_name.localeCompare(b.template_name)); break
    case 1: list = [...list].sort((a, b) => b.template_name.localeCompare(a.template_name)); break
    case 2: list = [...list].sort((a, b) => b.best_run_date - a.best_run_date); break
    case 3: list = [...list].sort((a, b) => a.best_run_date - b.best_run_date); break
  }

  return list
})

function cycleSort() {
  sortIndex.value = (sortIndex.value + 1) % 4
}
</script>

<template>
  <!-- Если выбрана статистика — показываем её вместо списка -->
  <RaceStatistic
    v-if="selectedSummary"
    :summary="selectedSummary"
    :nickname="props.nickname ?? ''"
    @close="closeStatistic"
    @deleted="loadSummaries"
  />

  <div v-else class="races">
    <div class="menu-bar">
      <input
        type="text"
        class="race-search"
        :placeholder="$t('search')"
        v-model="searchRaceText"
      >
      <button class="race-search-clear" @click="searchRaceText = ''">⌫</button>
      <button @click="cycleSort">{{ SORT_MODES[sortIndex] }}</button>
    </div>

    <div class="races-list">
      <div v-if="loading" class="status-msg">{{ $t('loading') }}</div>
      <div v-else-if="error" class="status-msg status-error">{{ error }}</div>
      <div v-else-if="filtered.length === 0" class="status-msg">{{ $t('no_races') }}</div>

      <div
        v-else
        v-for="s in filtered"
        :key="s.template_id"
        class="race"
      >
        <div class="raceName">{{ s.template_name }}</div>
        <p class="vertsep"></p>
        <div class="raceTime">{{ formatTime(s.best_time) }}</div>
        <p class="vertsep"></p>
        <div class="raceDate">{{ formatDate(s.best_run_date) }}</div>
        <p class="vertsep"></p>
        <button class="race-statistic-btn" @click="openStatistic(s)">{{ $t('statistic') }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.races, .races-list {
  display: flex;
  flex-direction: column;
  height: 100%;
}
.menu-bar {
  display: flex;
  flex-direction: row;
  flex-wrap: nowrap;
  width: 100%;
  min-height: 40px;
  height: 40px;
  padding: 5px 0px 5px 10px;
  z-index: 10;
  gap: 10px;
  box-shadow: 3px 3px 0 0 rgba(0,0,0,0.5);
}

.race-search {
  text-align: center;
  height: 30px;
}
.race-search-clear {
  margin-left: -10px;
}

.races-list {
  overflow-y: auto;
}
.race {
  display: flex;
  flex-direction: row;
  width: fit-content;
  margin: 10px 0px 0px 10px;
  height: 30px;
  align-items: center;
  background-color: var(--card-bg);
  box-shadow: 0 0 0 2px rgba(0,0,0,0.5);
}
.race > div, .race > button {
  text-wrap: nowrap;
  padding: 0 10px;
}
.race > button {
  height: 30px;
}
.race-statistic-btn {
  padding: 0px 10px;
}

.status-msg {
  margin: 20px 0 0 10px;
  opacity: 0.6;
  font-size: 0.9em;
}
.status-error {
  color: var(--error, #e74c3c);
  opacity: 1;
}
</style>