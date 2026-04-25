<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'
import {
  Chart,
  LineController,
  LineElement,
  PointElement,
  LinearScale,
  CategoryScale,
  Tooltip,
  type TooltipItem,
} from 'chart.js'

Chart.register(LineController, LineElement, PointElement, LinearScale, CategoryScale, Tooltip)

const { t } = useI18n()
const STORAGE_KEY_STATISTIC_SORT = 'race_statistic_sort_index'

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    emit('close')
  }
}

interface TemplateSummary {
  template_id: string
  template_name: string
  best_time: number
  best_run_date: number
  runs_count: number
  abort_count: number
}

interface Split {
  split_index: number
  split_name: string
  split_time: number
}

interface Run {
  id: number
  total_time: number
  created_at: number
  splits: Split[]
}

interface UserTemplate {
  id: string
  name: string
}

const userTemplates = ref<UserTemplate[]>([])
const syncLoading = ref(false)

async function loadUserTemplates() {
  try {
    const all = await invoke<UserTemplate[]>('get_templates')
    userTemplates.value = all.filter(t => t.id !== props.summary.template_id)
  } catch (e) {
    console.error(e)
  }
}

async function syncToTemplate(target: UserTemplate) {
  syncLoading.value = true
  try {
    await invoke('sync_runs_to_template', {
      nickname: props.nickname,
      fromTemplateId: props.summary.template_id,
      toTemplateId: target.id,
      toTemplateName: target.name,
    })
    emit('deleted')
    emit('close')
  } catch (e) {
    console.error(e)
  } finally {
    syncLoading.value = false
  }
}

onMounted(async () => {
  window.addEventListener('keydown', handleKeyDown)
  loadRuns()
  loadUserTemplates()
  unlistenRunSaved = await listen<number>('run-saved', async () => {
    await loadRuns()
    emit('deleted')
  })
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown)
  if (unlistenRunSaved) unlistenRunSaved()
  if (chartInstance) { chartInstance.destroy(); chartInstance = null }
})

const props = defineProps<{
  summary: TemplateSummary
  nickname: string
}>()

const emit = defineEmits<{
  close: []
  deleted: []
}>()

const runs = ref<Run[]>([])
const loading = ref(false)
const pendingDeleteId = ref<number | null>(null)
const searchText = ref('')
const sortIndex = ref(parseInt(localStorage.getItem(STORAGE_KEY_STATISTIC_SORT) || '0'))

const SORT_MODES = computed(() => [
  t('sort_best') + ' ⇩',
  t('sort_best') + ' ⇧',
  t('sort_date') + ' ⇩',
  t('sort_date') + ' ⇧',
])

function cycleSort() {
  sortIndex.value = (sortIndex.value + 1) % 4
}

const localAbortCount = ref(props.summary.abort_count)

async function loadRuns() {
  loading.value = true
  try {
    runs.value = await invoke<Run[]>('get_runs', {
      nickname: props.nickname,
      templateId: props.summary.template_id,
    })
  } catch (e) {
    console.error(e)
  } finally {
    loading.value = false
  }
}

onMounted(async () => {
  loadRuns()
  unlistenRunSaved = await listen<number>('run-saved', async () => {
    await loadRuns()
    emit('deleted')

    try {
    const summaries = await invoke<Array<{
      template_id: string
      abort_count: number
    }>>('get_template_summaries', { nickname: props.nickname })
    const match = summaries.find(
      s => s.template_id === props.summary.template_id
    )
    if (match) localAbortCount.value = match.abort_count
  } catch {}
  })
})

const sumOfBest = computed((): number | null => {
  if (runs.value.length === 0) return null

  const bestSegments = new Map<number, number>()

  for (const run of runs.value) {
    const sorted = [...run.splits].sort((a, b) => a.split_index - b.split_index)
    for (let i = 0; i < sorted.length; i++) {
      const segment = i === 0
        ? sorted[i].split_time
        : sorted[i].split_time - sorted[i - 1].split_time
      const current = bestSegments.get(sorted[i].split_index)
      if (current === undefined || segment < current) {
        bestSegments.set(sorted[i].split_index, segment)
      }
    }
  }

  if (bestSegments.size === 0) return null
  let total = 0
  for (const duration of bestSegments.values()) total += duration
  return total
})

function askDelete(id: number) {
  pendingDeleteId.value = id
}

function cancelDelete() {
  pendingDeleteId.value = null
}

async function confirmDelete(id: number) {
  try {
    await invoke('delete_run', { runId: id })
    runs.value = runs.value.filter(r => r.id !== id)
    pendingDeleteId.value = null
    if (runs.value.length === 0) {
      emit('deleted')
      emit('close')
    } else {
      emit('deleted')
    }
  } catch (e) {
    console.error(e)
  }
}

function formatRunTime(sec: number): string {
  const totalMs  = Math.round(sec * 1000)
  const ms       = totalMs % 1000
  const totalSec = Math.floor(totalMs / 1000)
  const s        = totalSec % 60
  const totalMin = Math.floor(totalSec / 60)
  const m        = totalMin % 60
  const h        = Math.floor(totalMin / 60)

  const parts: string[] = []
  if (h > 0) parts.push(`${h}`+t('hours'))
  if (m > 0) parts.push(`${m}`+t('minutes'))
  if (s > 0 || parts.length === 0) parts.push(`${s}`+t('seconds'))
  if (ms > 0) parts.push(`${String(ms).padStart(3, '0')}`+t('milliseconds'))

  return parts.join(' ')
}

function formatTimeOfDay(ts: number): string {
  const d = new Date(ts * 1000)
  const hours = String(d.getHours()).padStart(2, '0')
  const minutes = String(d.getMinutes()).padStart(2, '0')
  return `${hours}:${minutes}`
}

function formatDate(ts: number): string {
  const d  = new Date(ts * 1000)
  const dd = String(d.getDate()).padStart(2, '0')
  const mm = String(d.getMonth() + 1).padStart(2, '0')
  const yy = String(d.getFullYear()).slice(2)
  return `${dd}.${mm}.${yy}`
}

const bestSplitTimes = computed(() => {
  const map = new Map<number, number>()
  for (const run of runs.value) {
    for (const split of run.splits) {
      const current = map.get(split.split_index)
      if (current === undefined || split.split_time < current) {
        map.set(split.split_index, split.split_time)
      }
    }
  }
  return map
})

function isBestSplitTime(splitIndex: number, splitTime: number): boolean {
  const best = bestSplitTimes.value.get(splitIndex)
  return best !== undefined && splitTime === best
}

const filteredRuns = computed(() => {
  const q = searchText.value.trim().toLowerCase()

  let list = runs.value.filter(run => {
    if (!q) return true
    if (formatRunTime(run.total_time).toLowerCase().includes(q)) return true
    if (formatDate(run.created_at).includes(q)) return true
    for (const split of run.splits) {
      if (split.split_name.toLowerCase().includes(q)) return true
      if (formatRunTime(split.split_time).toLowerCase().includes(q)) return true
    }
    return false
  })

  switch (sortIndex.value) {
    case 0: list = [...list].sort((a, b) => a.total_time - b.total_time); break
    case 1: list = [...list].sort((a, b) => b.total_time - a.total_time); break
    case 2: list = [...list].sort((a, b) => b.created_at - a.created_at); break
    case 3: list = [...list].sort((a, b) => a.created_at - b.created_at); break
  }

  return list
})

const chartRuns = computed(() =>
  [...runs.value].sort((a, b) => a.created_at - b.created_at)
)

const CHART_MIN_VISIBLE = 15
const chartVisible = ref(CHART_MIN_VISIBLE)
const chartStart = ref(0)

watch(chartRuns, (runs) => {
  chartStart.value = Math.max(0, runs.length - chartVisible.value)
})

const visibleChartRuns = computed(() => {
  const total = chartRuns.value.length
  const visible = Math.min(chartVisible.value, total)
  const start = Math.min(chartStart.value, Math.max(0, total - visible))
  const end = start + visible
  return chartRuns.value.slice(start, end)
})

const chartCanvas = ref<HTMLCanvasElement | null>(null)
  let chartInstance: Chart | null = null
  
  let unlistenRunSaved: UnlistenFn | null = null
  
  function getCssVar(name: string): string {
    return getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim()
  }
  const text   = getCssVar('--text-color')
  
const chartHeight = ref(247)
const yTicksLimit = computed(() => Math.max(2, Math.floor(chartHeight.value / 20)))
function onResizerMouseDown(e: MouseEvent) {
  e.preventDefault()
  const startY = e.clientY
  const startHeight = chartHeight.value

  const onMove = (e: MouseEvent) => {
    const delta = e.clientY - startY
    chartHeight.value = Math.max(247, startHeight + delta)
  }

  const onUp = () => {
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
    nextTick(() => buildChart())
  }

  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

function buildChart() {
  if (!chartCanvas.value || visibleChartRuns.value.length === 0) return

  if (chartInstance) {
    chartInstance.destroy()
    chartInstance = null
  }

  const labels = visibleChartRuns.value.map(r => {
    const globalIndex = chartRuns.value.findIndex(cr => cr.id === r.id)
    return `${globalIndex + 1}`
  })

  const splitIndexes = new Set<number>()
  visibleChartRuns.value.forEach(run => {
    run.splits.forEach(s => {
      if (s.split_index !== 0) {
        splitIndexes.add(s.split_index)
      }
    })
  })

  const datasets = []

  const totalData = visibleChartRuns.value.map(r => r.total_time)
  const globalMinTime = Math.min(...chartRuns.value.map(r => r.total_time))
  const maxSplitIndex = Math.max(...Array.from(splitIndexes))

  datasets.push({
    label: 'Total',
    data: totalData,
    borderColor: '#90eebb',
    backgroundColor: 'rgba(144, 238, 144, 0.1)',
    pointBackgroundColor: totalData.map(t => t === globalMinTime ? '#ffd700' : '#90ee90'),
    pointBorderColor: totalData.map(t => t === globalMinTime ? '#ffd700' : '#90ee90'),
    pointRadius: 4,
    tension: 0.3,
  })

  splitIndexes.forEach(splitIndex => {
    if (splitIndex === maxSplitIndex) return

    const splitName =
      visibleChartRuns.value.find(r =>
        r.splits.find(s => s.split_index === splitIndex)
      )?.splits.find(s => s.split_index === splitIndex)?.split_name || `Split ${splitIndex}`

    const splitData = visibleChartRuns.value.map(run => {
      const split = run.splits.find(s => s.split_index === splitIndex)
      return split ? split.split_time : null
    })

    const globalBestTime = Math.min(
      ...chartRuns.value
        .map(run => run.splits.find(s => s.split_index === splitIndex)?.split_time)
        .filter((t): t is number => t !== undefined)
    )

    const pointColors = splitData.map(t =>
      t !== null && t === globalBestTime ? '#ffd700' : text
    )

    datasets.push({
      label: splitName,
      data: splitData,
      borderColor: 'rgba(125,125,125,0.5)',
      backgroundColor: 'transparent',
      pointBackgroundColor: pointColors,
      pointBorderColor: pointColors,
      pointRadius: 3,
      tension: 0.3,
    })
  })

  chartInstance = new Chart(chartCanvas.value, {
    type: 'line',
    data: {
      labels,
      datasets,
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      animation: false,
      interaction: {
        mode: 'nearest',
        intersect: true,
      },
      plugins: {
        legend: { display: false },
        tooltip: {
          titleAlign: 'center',
          displayColors: false,
          callbacks: {
            title(items: TooltipItem<'line'>[]) {
              const item = items[0]
              const datasetLabel = item.dataset.label
              if (datasetLabel === 'Total') {
                const lastSplit = visibleChartRuns.value[0]?.splits.slice(-1)[0]
                return lastSplit?.split_name
              }
              return datasetLabel
            },
            label(item: TooltipItem<'line'>) {
              const splitTime = formatRunTime(item.raw as number)
              return `${splitTime}`
            },
          },
        },
      },
      scales: {
        x: {
          ticks: { color: text, font: { size: 11 } },
          grid:  { color: 'rgba(125,125,125,0.3)' },
        },
        y: {
          ticks: {
            color: text,
            font: { size: 11 },
            maxTicksLimit: yTicksLimit.value,
            callback: (val) => formatRunTime(val as number),
          },
          grid: { color: 'rgba(125,125,125,0.3)' },
        },
      },
    },
  })
}

watch(visibleChartRuns, async () => {
  await nextTick()
  buildChart()
}, { deep: true })

watch(sortIndex, (newVal) => {
  localStorage.setItem(STORAGE_KEY_STATISTIC_SORT, newVal.toString())
})

watch(yTicksLimit, () => {
  buildChart()
})

onUnmounted(() => {
  if (unlistenRunSaved) unlistenRunSaved()
  if (chartInstance) {
    chartInstance.destroy()
    chartInstance = null
  }
})
</script>

<template>
  <div class="race-statistic">
    <div class="template-title" v-if="summary.template_name, runs.length >= 2" >
      <span>{{ summary.template_name }}</span>
      <span v-if="sumOfBest !== null">{{ $t('sum_of_best') }}: {{ formatRunTime(sumOfBest) }}</span>
      <span>{{ $t('failures') }}: {{ localAbortCount  }}</span>
    </div>

    <!-- График — показываем только если есть хотя бы 2 рана -->
    <div class="chart-container" v-if="runs.length >= 2" :style="{ height: chartHeight + 'px' }">
      <canvas ref="chartCanvas"></canvas>
    </div>
    
    <div class="chart-controls" v-if="chartRuns.length > CHART_MIN_VISIBLE">
      <div class="chart-control-row">
        <img src="../../imgs/zoom.png" width="15px" height="15px">
        <input
          type="range"
          :min="CHART_MIN_VISIBLE"
          :max="chartRuns.length"
          v-model.number="chartVisible"
          @input="chartStart = Math.min(chartStart, Math.max(0, chartRuns.length - chartVisible))"
        >
      </div>
      <div class="chart-control-row">
        <img src="../../imgs/toLeft.png" width="12px" height="12px">
        <input
        type="range"
        :min="0"
        :max="Math.max(0, chartRuns.length - chartVisible)"
        v-model.number="chartStart"
        >
        <img src="../../imgs/toRight.png" width="12px" height="12px">
      </div>
    </div>

    <div
      class="chart-resizer"
      @mousedown="onResizerMouseDown"
      v-if="runs.length >= 2"
    >
      <span class="chart-resizer-dots">• • •</span>
    </div>

    <div class="race-menu">
      <input
        type="text"
        class="race-history-search"
        :placeholder="$t('search')"
        v-model="searchText"
      >
      <button class="race-history-search-clear" @click="searchText = ''">⌫</button>
      <button @click="cycleSort">{{ SORT_MODES[sortIndex] }}</button>
      <div class="sync-wrapper" v-if="userTemplates.length > 0">
        <button class="sync-btn">
          <img class="sync-img" src="../../imgs/sync.png">
        </button>
        <select
          :title="$t('sync_tooltip')"
          class="sync-select"
          @change="(e) => {
            const target = e.target as HTMLSelectElement
            const tmpl = userTemplates.find(t => t.id === target.value)
            if (tmpl) syncToTemplate(tmpl)
            target.value = ''
          }"
        >
          <option
            v-for="tmpl in userTemplates"
            :key="tmpl.id"
            :value="tmpl.id"
          >{{ tmpl.name }}</option>
        </select>
      </div>
    </div>

    <div class="history-list">
      <div v-if="loading" class="status-msg">{{ $t('loading') }}</div>
      <div v-else-if="filteredRuns.length === 0" class="status-msg">{{ $t('no_races') }}</div>

      <div
        v-else
        v-for="run in filteredRuns"
        :key="run.id"
        class="history-record"
      >
        <div class="record-date">
          <div class="rtime">{{ formatTimeOfDay(run.created_at) }}</div>
          <div class="rdate">{{ formatDate(run.created_at) }}</div>
        </div>

        <template v-for="(split, idx) in run.splits" :key="split.split_index">
          <template v-if="idx !== 0">
            <p class="vertsep"></p>
            <div class="record-split">
              <div class="record-split-name">{{ split.split_name }}</div>
              <p class="horsep"></p>
              <div
                class="record-split-time"
                :class="{ 'best-split': isBestSplitTime(split.split_index, split.split_time) }"
              >
                {{ formatRunTime(split.split_time) }}
              </div>
            </div>
          </template>
        </template>

        <p class="vertsep"></p>

        <button
          class="record-delete"
          :disabled="pendingDeleteId === run.id"
          @click="askDelete(run.id)"
        >
          <img src="../../imgs/garbage.png" width="30px" height="30px">
        </button>

        <div class="deletion-record" v-if="pendingDeleteId === run.id">
          <button class="confirm-deletion-record" @click="confirmDelete(run.id)">✔</button>
          <button class="cancel-deletion-record" @click="cancelDelete()">✖</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.race-statistic {
  height: 100%;
  display: flex;
  flex-direction: column;
  width: 100%;
  overflow: hidden;
}
.race-menu {
  display: flex;
  flex-direction: row;
  flex-wrap: nowrap;
  width: 100%;
  height: 40px;
  margin-top: 3px;
  padding: 5px 0px 5px 10px;
  z-index: 5;
  gap: 10px;
  box-shadow: 3px 3px 0 0 rgba(0,0,0,0.5);
  flex-shrink: 0;
}
.race-history-search {
  text-align: center;
}
.race-history-search-clear {
  margin-left: -20px;
}
.sync-wrapper {
  position: relative;
  display: inline-flex;
}
.sync-btn {
  height: 100%;
  pointer-events: none;
}
.sync-img{
  width: 25px;
  height: 25px;
  margin-top: 1px;
}
.sync-select {
  position: absolute;
  inset: 0;
  opacity: 0;
  cursor: pointer;
  width: 100%;
}

.template-title {
  display: flex;
  flex-direction: row;
  margin-bottom: -15px;
  margin-top: 5px;
  align-self: center;
  gap: 50px;
}

.chart-container {
  width: 100%;
  padding: 10px 10px 5px 10px;
}
.chart-container canvas {
  width: 100% !important;
  height: 100% !important;
}
.chart-resizer {
  width: 100%;
  height: 14px;
  cursor: ns-resize;
  background: rgba(128, 128, 128, 0.5);
  box-shadow: 0px 3px 0 0 rgba(0,0,0,0.5);
  margin-left: 3px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s;
  user-select: none;
}
.chart-resizer-dots {
  letter-spacing: 3px;
  pointer-events: none;
}
.chart-controls {
  display: flex;
  flex-direction: row;
  width: 100%;
  align-self: center;
  gap: 50px;
  padding: 0 15px 5px 15px;
  margin-top: -5px;
}
.chart-control-row {
  display: flex;
  width: 100%;
  align-items: center;
  font-size: 11px;
}
.chart-control-row input[type="range"] {
  flex: 1;
}

.history-list {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  padding-bottom: 10px;
  min-height: 0;
}
.history-record {
  display: flex;
  flex-direction: row;
  height: max-content;
  width: max-content;
  margin: 10px 40px 0px 10px;
  text-align: center;
  background-color: var(--card-bg);
  box-shadow: 0 0 0 2px rgba(0,0,0,0.5);
  align-items: center;
}
.history-record > div {
  height: 100%;
  align-content: center;
}
.record-split {
  display: flex;
  flex-direction: column;
  justify-content: center;
}
.rtime {
  padding: 5px;
}
.rdate {
  font-size: 15px;
  padding: 5px;
}
.record-date > div,
.record-split > div {
  padding: 3px 5px;
}
.record-split-time {
  font-size: 17px;
}
.best-split {
  color: rgba(255, 255, 0);
  text-shadow: 1px 1px 2px black;
}

.record-delete {
  height: 100%;
  padding: 0 5px;
  display: flex;
  align-items: center;
}
.record-delete:disabled {
  opacity: 0.4;
  cursor: default;
}

.deletion-record {
  display: flex;
  flex-direction: column;
  height: 100%;
}
.deletion-record > button {
  flex: 1;
  min-width: 32px;
}
.confirm-deletion-record {
  color: #90ee90;
}
.cancel-deletion-record {
  color: #e05050;
}

.status-msg {
  margin: 20px 0 0 10px;
  opacity: 0.6;
  font-size: 0.9em;
}
</style>
