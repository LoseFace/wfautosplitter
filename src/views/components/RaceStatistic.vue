<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, emit as tauriEmit, type UnlistenFn } from '@tauri-apps/api/event'
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
  type Plugin,
  type ChartEvent,
} from 'chart.js'

import imgZoom from '../../imgs/zoom.png'
import imgToLeft from '../../imgs/toLeft.png'
import imgToRight from '../../imgs/toRight.png'
import imgGarbage from '../../imgs/garbage.png'

Chart.register(LineController, LineElement, PointElement, LinearScale, CategoryScale, Tooltip)

const { t } = useI18n()

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
  success: boolean
}

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown)
  if (unlistenRunSaved) unlistenRunSaved()
  if (chartInstance) { chartInstance.destroy(); chartInstance = null }
})

const props = defineProps<{
  summary: TemplateSummary
}>()

const emit = defineEmits<{
  close: []
  deleted: []
}>()

const runs = ref<Run[]>([])
const loading = ref(false)
const pendingDeleteId = ref<number | null>(null)

type SortDir = 'asc' | 'desc'
const lastSplitIndex = computed(() => {
  const cols = splitColumns.value
  return cols.length > 0 ? cols[cols.length - 1].index : null
})
const sortCol = ref<string>('date')
const sortDir = ref<SortDir>('desc')
const showSegments = ref(localStorage.getItem('race_show_segments') === 'true')

watch(showSegments, (val) => {
  localStorage.setItem('race_show_segments', String(val))
})

function toggleSort(col: string) {
  if (sortCol.value === col) {
    sortDir.value = sortDir.value === 'asc' ? 'desc' : 'asc'
  } else {
    sortCol.value = col
    sortDir.value = 'asc'
  }
}

function sortIcon(col: string): string {
  const resolvedActive = sortCol.value === '__last__'
    ? (lastSplitIndex.value !== null ? 'split:' + lastSplitIndex.value : 'date')
    : sortCol.value
  if (resolvedActive !== col) return '⇅'
  return sortDir.value === 'asc' ? '⇩' : '⇧'
}

const activeSortCol = computed(() =>
  sortCol.value === '__last__'
    ? (lastSplitIndex.value !== null ? 'split:' + lastSplitIndex.value : 'date')
    : sortCol.value
)

const localAbortCount = ref(props.summary.abort_count)

async function loadRuns() {
  loading.value = true
  try {
    runs.value = await invoke<Run[]>('get_runs', {
      templateId: props.summary.template_id,
    })
  } catch (e) {
    console.error(e)
  } finally {
    loading.value = false
  }
}

onMounted(async () => {
  window.addEventListener('keydown', handleKeyDown)
  loadRuns()
  unlistenRunSaved = await listen<number>('run-saved', async () => {
    await loadRuns()
    emit('deleted')

    try {
    const summaries = await invoke<Array<{
      template_id: string
      abort_count: number
    }>>('get_template_summaries', {})
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

const totalColumns = computed(() => {
  const visibleSplits = splitColumns.value.filter((_, i) => i > 0).length
  const segmentCols = showSegments.value ? Math.max(0, visibleSplits - 1) : 0
  return 1 + visibleSplits + segmentCols + 1
})

async function confirmDelete(id: number) {
  try {
    const runToDelete = runs.value.find(r => r.id === id)
    await invoke('delete_run', { runId: id })
    runs.value = runs.value.filter(r => r.id !== id)
    pendingDeleteId.value = null

    if (runToDelete && !runToDelete.success) {
      localAbortCount.value = Math.max(0, localAbortCount.value - 1)
      await tauriEmit('abort-decremented', {})
    }

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

const bestSegmentTimes = computed(() => {
  const map = new Map<string, number>()
  const cols = splitColumns.value.filter(c => c.index > 0)
  for (let i = 1; i < cols.length; i++) {
    const fromIdx = cols[i - 1].index
    const toIdx = cols[i].index
    const key = `${fromIdx}:${toIdx}`
    for (const run of runs.value) {
      const t = getSegmentTime(run, fromIdx, toIdx)
      if (t === null) continue
      const cur = map.get(key)
      if (cur === undefined || t < cur) map.set(key, t)
    }
  }
  return map
})

function isBestSplitTime(splitIndex: number, splitTime: number): boolean {
  const best = bestSplitTimes.value.get(splitIndex)
  return best !== undefined && splitTime === best
}

const filteredRuns = computed(() => {
  let list = runs.value

  const col = sortCol.value
  const dir = sortDir.value

  const resolvedCol = col === '__last__'
    ? (lastSplitIndex.value !== null ? 'split:' + lastSplitIndex.value : 'date')
    : col

  if (resolvedCol === 'date') {
    list = [...list].sort((a, b) =>
      dir === 'desc' ? b.created_at - a.created_at : a.created_at - b.created_at
    )
  } else if (resolvedCol.startsWith('split:')) {
    const splitIdx = parseInt(resolvedCol.slice(6))
    list = [...list].sort((a, b) => {
      const sa = a.splits.find(s => s.split_index === splitIdx)
      const sb = b.splits.find(s => s.split_index === splitIdx)
      if (!sa && !sb) return 0
      if (!sa) return 1
      if (!sb) return -1
      const diff = sa.split_time - sb.split_time
      return dir === 'asc' ? diff : -diff
    })
  } else if (resolvedCol.startsWith('seg:')) {
    const parts = resolvedCol.split(':')
    const fromIdx = parseInt(parts[1])
    const toIdx = parseInt(parts[2])
    list = [...list].sort((a, b) => {
      const segA = getSegmentTime(a, fromIdx, toIdx)
      const segB = getSegmentTime(b, fromIdx, toIdx)
      if (segA === null && segB === null) return 0
      if (segA === null) return 1
      if (segB === null) return -1
      return dir === 'asc' ? segA - segB : segB - segA
    })
  }

  return list
})

const splitColumns = computed(() => {
  const map = new Map<number, string>()
  for (const run of runs.value) {
    for (const split of run.splits) {
      if (!map.has(split.split_index)) {
        map.set(split.split_index, split.split_name)
      }
    }
  }
  return [...map.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([index, name]) => ({ index, name }))
})

function isFailCell(run: Run, colIndex: number): boolean {
  if (run.success) return false
  const visibleCols = splitColumns.value.filter(c => c.index > 0)
  const lastReached = [...visibleCols]
    .reverse()
    .find(c => run.splits.some(s => s.split_index === c.index))
  if (!lastReached) {
    return colIndex === visibleCols[0]?.index
  }
  const lastIdx = visibleCols.findIndex(c => c.index === lastReached.index)
  const nextCol = visibleCols[lastIdx + 1]
  return nextCol?.index === colIndex
}

function getSegmentTime(run: Run, fromIndex: number, toIndex: number): number | null {
  const fromSplit = run.splits.find(s => s.split_index === fromIndex)
  const toSplit = run.splits.find(s => s.split_index === toIndex)
  if (!fromSplit || !toSplit) return null
  return toSplit.split_time - fromSplit.split_time
}

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

function onChartWheel(e: WheelEvent) {
  const maxStart = Math.max(0, chartRuns.value.length - chartVisible.value)
  if (e.deltaY < 0) {
    chartStart.value = Math.max(0, chartStart.value - 1)
  } else {
    chartStart.value = Math.min(maxStart, chartStart.value + 1)
  }
}

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
      if (s.split_index !== 0) splitIndexes.add(s.split_index)
    })
  })

  const datasets = []
  const totalData = visibleChartRuns.value.map(r => r.success ? r.total_time : null)
  const globalMinTime = Math.min(
    ...chartRuns.value
      .filter(r => r.success)
      .map(r => r.total_time)
  )
  const maxSplitIndex = Math.max(...Array.from(splitIndexes))

  const lastSplitName = visibleChartRuns.value
    .filter(r => r.success)
    .flatMap(r => r.splits)
    .sort((a, b) => b.split_index - a.split_index)[0]?.split_name

  datasets.push({
    label: lastSplitName,
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
      visibleChartRuns.value.find(r => r.splits.find(s => s.split_index === splitIndex))
        ?.splits.find(s => s.split_index === splitIndex)?.split_name || `Split ${splitIndex}`

    const splitData = visibleChartRuns.value.map(run => {
      const split = run.splits.find(s => s.split_index === splitIndex)
      return split ? split.split_time : null
    })

    const globalBestTime = Math.min(
      ...chartRuns.value
        .filter(r => r.success)
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

  let segmentTooltip: {
    datasetIndex: number
    pointIndex: number
    x: number
    y: number
    label: string
  } | null = null

  const segmentTooltipPlugin: Plugin<'line'> = {
    id: 'segmentTooltip',
    afterEvent(chart, args) {
      const event: ChartEvent = args.event
      if (event.type !== 'mousemove' && event.type !== 'mouseout') return

      if (event.type === 'mouseout') {
        segmentTooltip = null
        chart.draw()
        return
      }

      const mx = event.x
      const my = event.y
      if (mx === null || my === null) return

      const chartArea = chart.chartArea
      if (mx < chartArea.left || mx > chartArea.right || my < chartArea.top || my > chartArea.bottom) {
        if (segmentTooltip) { segmentTooltip = null; chart.draw() }
        return
      }

      let found: typeof segmentTooltip = null

      const meta0 = chart.getDatasetMeta(0)
      if (!meta0.data.length) return

      let closestRunIndex = 0
      let closestDx = Infinity
      for (let i = 0; i < meta0.data.length; i++) {
        const dx = Math.abs(meta0.data[i].x - mx)
        if (dx < closestDx) { closestDx = dx; closestRunIndex = i }
      }

      if (closestDx > (chart.chartArea.width / meta0.data.length) * 0.1) {
        if (segmentTooltip) { segmentTooltip = null; chart.draw() }
        return
      }

      const run = visibleChartRuns.value[closestRunIndex]
      if (!run) return

      const sorted = [...run.splits].sort((a, b) => a.split_index - b.split_index)

      const splitPoints: { splitIndex: number; splitName: string; splitTime: number; y: number }[] = []

      for (let di = 1; di < chart.data.datasets.length; di++) {
        const meta = chart.getDatasetMeta(di)
        const point = meta.data[closestRunIndex]
        if (!point) continue
        const dataset = chart.data.datasets[di]
        const splitName = dataset.label as string
        const split = sorted.find(s => s.split_name === splitName || `Split ${s.split_index}` === splitName)
        if (!split) continue
        splitPoints.push({
          splitIndex: split.split_index,
          splitName: split.split_name,
          splitTime: split.split_time,
          y: point.y,
        })
      }

      const meta0point = meta0.data[closestRunIndex]
      if (meta0point) {
        const lastActualSplit = sorted[sorted.length - 1]
        splitPoints.push({
          splitIndex: 999999,
          splitName: lastActualSplit?.split_name,
          splitTime: run.total_time,
          y: meta0point.y,
        })
      }

      splitPoints.sort((a, b) => b.y - a.y)

      const margin = 6
      for (let i = 1; i < splitPoints.length; i++) {
        const bottom = splitPoints[i - 1]
        const top = splitPoints[i]
        const yMin = Math.min(bottom.y, top.y) + margin
        const yMax = Math.max(bottom.y, top.y) - margin

        if (my >= yMin && my <= yMax) {
          const segTime = top.splitTime - bottom.splitTime

          const fromName = bottom.splitName
          const toName = top.splitName

          found = {
            datasetIndex: 0,
            pointIndex: closestRunIndex,
            x: meta0.data[closestRunIndex].x,
            y: (bottom.y + top.y) / 2,
            label: `${fromName} → ${toName}\n${formatRunTime(segTime)}`,
          }
          break
        }
      }

      const changed = JSON.stringify(found) !== JSON.stringify(segmentTooltip)
      segmentTooltip = found
      if (changed) chart.draw()
    },

    afterDraw(chart) {
      if (!segmentTooltip) return

      const ctx = chart.ctx
      const { x, y, label } = segmentTooltip
      const lines = label.split('\n')
      const padding = 6
      const lineHeight = 16
      
      ctx.save()
      ctx.font = '12px sans-serif'
      const maxW = Math.max(...lines.map((l: string) => ctx.measureText(l).width))
      const boxW = maxW + padding * 2
      const boxH = lines.length * lineHeight + padding * 2 - 5

      let bx = x + 10
      if (bx + boxW > chart.chartArea.right) bx = x - boxW - 10

      const by = y - boxH / 2

      ctx.fillStyle = 'rgba(0,0,0,0.8)'
      ctx.beginPath()
      ctx.roundRect(bx, by, boxW, boxH, 4)
      ctx.fill()
      ctx.fillStyle = 'rgba(255,255,255)'
      ctx.textAlign = 'center'
      ctx.textBaseline = 'top'
      const textX = bx + boxW / 2
      lines.forEach((line: string, i: number) => {
        ctx.fillText(line, textX, by + padding + i * lineHeight)
      })

      ctx.restore()
    },
  }

  chartInstance = new Chart(chartCanvas.value, {
    type: 'line',
    data: { labels, datasets },
    plugins: [segmentTooltipPlugin],
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
                return items[0].dataset.label
              }
              return datasetLabel
            },
            label(item: TooltipItem<'line'>) {
              return formatRunTime(item.raw as number)
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

    <div
      class="chart-container"
      v-if="runs.length >= 2"
      :style="{ height: chartHeight + 'px' }"
      @wheel.prevent="onChartWheel"
    >
      <canvas ref="chartCanvas"></canvas>
    </div>
    
    <div class="chart-controls" v-if="chartRuns.length > CHART_MIN_VISIBLE">
      <div class="chart-control-row">
        <img :src="imgZoom" width="15px" height="15px">
        <input
          type="range"
          :min="CHART_MIN_VISIBLE"
          :max="chartRuns.length"
          v-model.number="chartVisible"
          @input="chartStart = Math.min(chartStart, Math.max(0, chartRuns.length - chartVisible))"
        >
      </div>
      <div class="chart-control-row">
        <img :src="imgToLeft" width="12px" height="12px">
        <input
          type="range"
          :min="0"
          :max="Math.max(0, chartRuns.length - chartVisible)"
          v-model.number="chartStart"
        >
        <img :src="imgToRight" width="12px" height="12px">
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
      <button class="segments-toggle" @click="showSegments = !showSegments" :class="{ active: showSegments }">
        {{ showSegments ? $t('hide_segments') : $t('show_segments') }}
      </button>
    </div>

    <div class="history-list">
      <div v-if="loading" class="status-msg">{{ $t('loading') }}</div>
      <div v-else-if="filteredRuns.length === 0" class="status-msg">{{ $t('no_races') }}</div>

      <div v-else class="history-table-wrap">
        <table class="history-table">
          <thead>
            <tr class="thead-splits">
              <th
                class="th-date sortable col-border-right"
                :class="{ 'col-active': activeSortCol === 'date' }"
                @click="toggleSort('date')"
              >
                {{ $t('date') }} <span class="sort-icon">{{ sortIcon('date') }}</span>
              </th>
              <template v-for="(col, idx) in splitColumns" :key="col.index">
                <th
                  v-if="showSegments && idx > 1"
                  class="th-segment sortable col-border-right"
                  :class="{ 'col-active': activeSortCol === 'seg:' + splitColumns[idx-1].index + ':' + col.index }"
                  @click="toggleSort('seg:' + splitColumns[idx-1].index + ':' + col.index)"
                >
                  <span class="seg-label">{{ splitColumns[idx-1].name }} → {{ col.name }}</span>
                  <span class="sort-icon">{{ sortIcon('seg:' + splitColumns[idx-1].index + ':' + col.index) }}</span>
                </th>
                <th
                  v-if="idx > 0"
                  class="th-split sortable col-border-right"
                  :class="{ 'col-active': activeSortCol === 'split:' + col.index }"
                  @click="toggleSort('split:' + col.index)"
                >
                  {{ col.name }} <span class="sort-icon">{{ sortIcon('split:' + col.index) }}</span>
                </th>
              </template>
              <th class="th-del"></th>
            </tr>
          </thead>
          <tbody>
            <template v-for="run in filteredRuns" :key="run.id">
              <tr v-if="pendingDeleteId === run.id" class="tr-confirm-delete">
                <td class="td-date col-border-right">
                  <div class="rtime">{{ formatTimeOfDay(run.created_at) }}</div>
                  <div class="rdate">{{ formatDate(run.created_at) }}</div>
                </td>
                <td :colspan="totalColumns - 1" class="td-confirm-delete">
                  <span class="confirm-delete-text">{{$t('delete_record')}}</span>
                  <button class="confirm-deletion-record" @click="confirmDelete(run.id)">{{$t('delete')}}</button>
                  <button class="cancel-deletion-record" @click="cancelDelete()">{{$t('cancel')}}</button>
                </td>
              </tr>
              <tr v-else class="tr-splits" :class="{ 'tr-failed': !run.success }">
                <td class="td-date col-border-right" :class="{ 'col-active-cell': activeSortCol === 'date' }">
                  <div class="rtime">{{ formatTimeOfDay(run.created_at) }}</div>
                  <div class="rdate">{{ formatDate(run.created_at) }}</div>
                </td>
                <template v-for="(col, idx) in splitColumns" :key="col.index">
                  <td
                    v-if="showSegments && idx > 1"
                    class="td-segment col-border-right"
                    :class="{
                      'col-active-cell': activeSortCol === 'seg:' + splitColumns[idx-1].index + ':' + col.index,
                      'best-split': (() => {
                        const t = getSegmentTime(run, splitColumns[idx-1].index, col.index)
                        const best = bestSegmentTimes.get(`${splitColumns[idx-1].index}:${col.index}`)
                        return t !== null && best !== undefined && t === best
                      })()
                    }"
                  >
                    <span v-if="getSegmentTime(run, splitColumns[idx-1].index, col.index) !== null">
                      {{ formatRunTime(getSegmentTime(run, splitColumns[idx-1].index, col.index)!) }}
                    </span>
                    <span v-else class="no-split">—</span>
                  </td>
                  <td
                    v-if="idx > 0"
                    class="td-split col-border-right"
                    :class="{
                      'best-split': (() => { const s = run.splits.find(sp => sp.split_index === col.index); return s ? isBestSplitTime(col.index, s.split_time) : false })(),
                      'col-active-cell': activeSortCol === 'split:' + col.index
                    }"
                  >
                    <template v-if="run.splits.find(sp => sp.split_index === col.index)">
                      {{ formatRunTime(run.splits.find(sp => sp.split_index === col.index)!.split_time) }}
                    </template>
                    <template v-else-if="isFailCell(run, col.index)">
                      <span class="run-failed-cell">{{ $t('fail') }}</span>
                    </template>
                    <span v-else class="no-split">—</span>
                  </td>
                </template>
                <td class="td-del">
                  <button
                    class="record-delete"
                    :disabled="pendingDeleteId === run.id"
                    @click="askDelete(run.id)"
                  >
                    <img :src="imgGarbage">
                  </button>
                </td>
              </tr>
            </template>
          </tbody>
        </table>
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
  overflow-y: auto;
  overflow-x: auto;
  min-height: 0;
}

.history-table-wrap {
  min-width: max-content;
}

.history-table {
  border-collapse: collapse;
  width: 100%;
  table-layout: auto;
}

.history-table thead th {
  position: sticky;
  top: 0;
  background: var(--card-bg);
  z-index: 2;
  padding: 12px 10px 10px 10px;
  white-space: nowrap;
  text-align: center;
  border-bottom: 2px solid rgba(127,127,127,0.5);
}

.th-segment {
  padding: 6px 6px !important;
}
.th-segment .seg-label {
  display: block;
  font-size: 15px;
}

.td-segment {
  text-align: center;
  white-space: nowrap;
}

.col-border-right {
  border-right: 1px solid rgba(127,127,127,0.5);
}

.col-active {
  background: var(--card-bg);
}
.col-active-cell {
  background: var(--card-bg);
}
.tr-splits:has(> .td-del:hover) .col-active-cell {
  background: transparent;
}

.sortable {
  cursor: pointer;
  user-select: none;
}
.sortable:hover {
  background: var(--btn-bg-color);
}
.sort-icon {
  display: inline-block;
  width: 1em;
}

.history-table tbody tr {
  border-bottom: 1px solid rgba(127,127,127,0.5);
}

.tr-splits td,
.tr-confirm-delete td{
  padding: 4px;
  text-align: center;
  vertical-align: middle;
  white-space: nowrap;
}
.tr-confirm-delete .td-confirm-delete{
  text-align: end;
}
.tr-splits:hover{
  background-color: var(--card-bg);
}
.tr-splits:has(> .td-del:hover) {
  background-color: rgba(255, 0, 0, 0.2) !important;
}

.rdate {
  font-size: 15px;
}

.best-split {
  color: rgba(255, 255, 0);
  text-shadow: 1px 1px 2px black;
}

.run-failed-cell {
  color: #e05050;
  font-weight: 600;
}

.td-del {
  padding: 0 !important;
  width: 36px;
  text-align: center;
  vertical-align: middle;
}

.record-delete {
  padding: 8px 3px 7px 2px;
  height: 100%;
  background-color: transparent !important;
}
.record-delete > img {
  width: 30px;
  height: 30px;
}

.td-confirm-delete > * {
  margin-right: 10px;
}
.confirm-deletion-record,
.cancel-deletion-record {
  height: 30px;
}
</style>