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
  group_index: number
}

interface SplitColumn {
  index: number
  name: string
  group_index: number
}

interface GroupBlock {
  group_index: number
  cols: SplitColumn[]
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

const showSplits = ref(localStorage.getItem('race_show_splits') !== 'false')
const showSegments = ref(localStorage.getItem('race_show_segments') === 'true')
const showGroupTotals = ref(localStorage.getItem('race_show_group_totals') !== 'false')


watch(showSplits, (val) => {
  localStorage.setItem('race_show_splits', String(val))
})
watch(showSegments, (val) => {
  localStorage.setItem('race_show_segments', String(val))
})
watch(showGroupTotals, (val) => {
  localStorage.setItem('race_show_group_totals', String(val))
})

function onShowSplitsChange() {
  if (!showSplits.value && !showSegments.value && !showGroupTotals.value) showGroupTotals.value = true
}
function onShowSegmentsChange() {
  if (!showSplits.value && !showSegments.value && !showGroupTotals.value) showSplits.value = true
}
function onShowGroupTotalsChange() {
  if (!showSplits.value && !showSegments.value && !showGroupTotals.value) showSplits.value = true
}

function getSequenceKey(run: Run): string {
  return run.splits
    .slice()
    .sort((a, b) => a.split_index - b.split_index)
    .map(s => s.split_name)
    .join('|')
}

function isPrefix(shorter: string, longer: string): boolean {
  if (shorter === longer) return true
  return longer.startsWith(shorter + '|')
}

const groupedRuns = computed(() => {
  const keyedRuns = runs.value.map(r => ({ run: r, key: getSequenceKey(r) }))
  const allKeys = [...new Set(keyedRuns.map(kr => kr.key))]

  function canonicalKey(key: string): string {
    let best = key
    for (const other of allKeys) {
      if (isPrefix(key, other) && other.length > best.length) {
        best = other
      }
    }
    return best
  }

  const map = new Map<string, Run[]>()
  for (const { run, key } of keyedRuns) {
    const canon = canonicalKey(key)
    if (!map.has(canon)) map.set(canon, [])
    map.get(canon)!.push(run)
  }

  return [...map.entries()]
    .map(([key, runs]) => ({
      key,
      runs,
      bestSplitTimes: getBestSplitTimes(runs),
      bestSegmentTimes: getBestSegmentTimes(runs),
      groupedSplitColumns: getGroupedSplitColumns(runs),
      totalColumns: getTotalColumns(runs),
      filteredRuns: [] as Run[],
    }))
    .sort((a, b) => {
      const aMax = Math.max(...a.runs.map(r => r.created_at))
      const bMax = Math.max(...b.runs.map(r => r.created_at))
      return bMax - aMax
    })
})

const processedGroups = computed(() =>
  groupedRuns.value.map(group => {
    const activeSortCol = getActiveSortCol(group.key, group.runs)
    const filteredRuns = getFilteredRuns(group.key, group.runs)
    const splitColumns = getSplitColumns(group.runs)
    return {
      ...group,
      filteredRuns,
      activeSortCol,
      processedRows: buildProcessedRows(
        filteredRuns,
        group.groupedSplitColumns,
        group.bestSplitTimes,
        group.bestSegmentTimes,
        splitColumns,
      ),
    }
  })
)

const tableSortStates = ref<Record<string, { col: string; dir: SortDir }>>({})

function getTableSort(key: string) {
  if (!tableSortStates.value[key]) {
    tableSortStates.value[key] = { col: 'date', dir: 'desc' }
  }
  return tableSortStates.value[key]
}

function toggleTableSort(key: string, col: string) {
  const state = getTableSort(key)
  if (state.col === col) {
    state.dir = state.dir === 'asc' ? 'desc' : 'asc'
  } else {
    state.col = col
    state.dir = 'asc'
  }
}

function getActiveSortCol(key: string, tableRuns: Run[]): string {
  const state = getTableSort(key)
  if (state.col !== '__last__') return state.col
  const cols = getSplitColumns(tableRuns)
  return cols.length > 0 ? 'split:' + cols[cols.length - 1].index : 'date'
}

function tableSortIcon(key: string, col: string, activeSortCol: string): string {
  if (activeSortCol !== col) return '⇅'
  return getTableSort(key).dir === 'asc' ? '⇩' : '⇧'
}

function getSplitColumns(tableRuns: Run[]) {
  const map = new Map<number, string>()
  for (const run of tableRuns) {
    for (const split of run.splits) {
      if (!map.has(split.split_index)) {
        map.set(split.split_index, split.split_name)
      }
    }
  }
  return [...map.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([index, name]) => ({ index, name }))
}

function getGroupedSplitColumns(tableRuns: Run[]): GroupBlock[] {
  const map = new Map<number, SplitColumn>()
  for (const run of tableRuns) {
    for (const split of run.splits) {
      if (!map.has(split.split_index)) {
        map.set(split.split_index, {
          index: split.split_index,
          name: split.split_name,
          group_index: split.group_index,
        })
      }
    }
  }
  const allCols = [...map.values()].sort((a, b) => a.index - b.index)

  const groupMap = new Map<number, SplitColumn[]>()
  for (const col of allCols) {
    if (!groupMap.has(col.group_index)) groupMap.set(col.group_index, [])
    groupMap.get(col.group_index)!.push(col)
  }

  return [...groupMap.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([group_index, cols]) => ({ group_index, cols }))
}

function getGroupTime(run: Run, group_index: number): number | null {
  const groupSplits = run.splits
    .filter(s => s.group_index === group_index)
    .sort((a, b) => a.split_index - b.split_index)
  if (groupSplits.length === 0) return null
  const last = groupSplits[groupSplits.length - 1]
  const first = groupSplits[0]
  const allSortedSplits = [...run.splits].sort((a, b) => a.split_index - b.split_index)
  const firstIdx = allSortedSplits.findIndex(s => s.split_index === first.split_index)
  const prevSplit = firstIdx > 0 ? allSortedSplits[firstIdx - 1] : null
  if (!prevSplit) return last.split_time
  return last.split_time - prevSplit.split_time
}

function getTotalColumns(tableRuns: Run[]): number {
  const cols = getSplitColumns(tableRuns)
  const visibleSplits = cols.filter((_, i) => i > 0).length
  const segmentCols = showSegments.value && showSplits.value ? Math.max(0, visibleSplits - 1) : 0
  const splitCols = showSplits.value ? visibleSplits : 0
  const groupBlocks = showGroupTotals.value ? getGroupedSplitColumns(tableRuns).length : 0
  return 1 + splitCols + segmentCols + groupBlocks + 1
}

function getBestSplitTimes(tableRuns: Run[]): Map<number, number> {
  const map = new Map<number, number>()
  for (const run of tableRuns) {
    for (const split of run.splits) {
      const cur = map.get(split.split_index)
      if (cur === undefined || split.split_time < cur) {
        map.set(split.split_index, split.split_time)
      }
    }
  }
  return map
}

function getBestSegmentTimes(tableRuns: Run[]): Map<string, number> {
  const map = new Map<string, number>()
  const cols = getSplitColumns(tableRuns).filter(c => c.index > 0)
  for (let i = 1; i < cols.length; i++) {
    const fromIdx = cols[i - 1].index
    const toIdx = cols[i].index
    const key = `${fromIdx}:${toIdx}`
    for (const run of tableRuns) {
      const seg = getSegmentTime(run, fromIdx, toIdx)
      if (seg === null) continue
      const cur = map.get(key)
      if (cur === undefined || seg < cur) map.set(key, seg)
    }
  }
  return map
}

function getFilteredRuns(key: string, tableRuns: Run[]): Run[] {
  const state = getTableSort(key)
  const cols = getSplitColumns(tableRuns)
  const lastIdx = cols.length > 0 ? cols[cols.length - 1].index : null
  const resolvedCol = state.col === '__last__'
    ? (lastIdx !== null ? 'split:' + lastIdx : 'date')
    : state.col
  const dir = state.dir

  let list = [...tableRuns]
  if (resolvedCol === 'date') {
    list.sort((a, b) => dir === 'desc' ? b.created_at - a.created_at : a.created_at - b.created_at)
  } else if (resolvedCol.startsWith('split:')) {
    const splitIdx = parseInt(resolvedCol.slice(6))
    list.sort((a, b) => {
      const sa = a.splits.find(s => s.split_index === splitIdx)
      const sb = b.splits.find(s => s.split_index === splitIdx)
      if (!sa && !sb) return 0
      if (!sa) return 1
      if (!sb) return -1
      return dir === 'asc' ? sa.split_time - sb.split_time : sb.split_time - sa.split_time
    })
  } else if (resolvedCol.startsWith('seg:')) {
    const parts = resolvedCol.split(':')
    const fromIdx = parseInt(parts[1])
    const toIdx = parseInt(parts[2])
    list.sort((a, b) => {
      const segA = getSegmentTime(a, fromIdx, toIdx)
      const segB = getSegmentTime(b, fromIdx, toIdx)
      if (segA === null && segB === null) return 0
      if (segA === null) return 1
      if (segB === null) return -1
      return dir === 'asc' ? segA - segB : segB - segA
    })
  }
  return list
}

interface ProcessedCell {
  segmentTime: number | null
  splitTime: number | null
  isBestSegment: boolean
  isBestSplit: boolean
  isFail: boolean
  groupTime: number | null
}

interface ProcessedRow {
  run: Run
  cells: Map<string, ProcessedCell>
}

function buildProcessedRows(
  tableRuns: Run[],
  groupedCols: GroupBlock[],
  bestSplitTimes: Map<number, number>,
  bestSegmentTimes: Map<string, number>,
  splitColumns: ReturnType<typeof getSplitColumns>
): ProcessedRow[] {
  return tableRuns.map(run => {
    const splitMap = new Map<number, number>()
    for (const s of run.splits) splitMap.set(s.split_index, s.split_time)

    const visibleCols = splitColumns.filter(c => c.index > 0)
    const lastReached = [...visibleCols].reverse().find(c => splitMap.has(c.index))
    const failColIndex = (() => {
      if (run.success) return null
      if (!lastReached) return visibleCols[0]?.index ?? null
      const lastIdx = visibleCols.findIndex(c => c.index === lastReached.index)
      return visibleCols[lastIdx + 1]?.index ?? null
    })()

    const cells = new Map<string, ProcessedCell>()

    for (const block of groupedCols) {
      for (let idx = 0; idx < block.cols.length; idx++) {
        const col = block.cols[idx]
        const key = `${block.group_index}:${col.index}`

        const splitTime = splitMap.get(col.index) ?? null

        let segmentTime: number | null = null
        let isBestSegment = false
        if (idx > 0) {
          const prevCol = block.cols[idx - 1]
          const fromTime = splitMap.get(prevCol.index)
          const toTime = splitMap.get(col.index)
          if (fromTime !== undefined && toTime !== undefined) {
            segmentTime = toTime - fromTime
            const bestSeg = bestSegmentTimes.get(`${prevCol.index}:${col.index}`)
            isBestSegment = bestSeg !== undefined && segmentTime === bestSeg
          }
        }

        const isBestSplit = splitTime !== null && splitTime === bestSplitTimes.get(col.index)
        const isFail = failColIndex === col.index

        const groupTime = getGroupTime(run, block.group_index)

        cells.set(key, { segmentTime, splitTime, isBestSegment, isBestSplit, isFail, groupTime })
      }
    }

    return { run, cells }
  })
}

function getSumOfBest(tableRuns: Run[]): number | null {
  if (tableRuns.length === 0) return null
  const bestSegments = new Map<number, number>()
  for (const run of tableRuns) {
    const sorted = [...run.splits].sort((a, b) => a.split_index - b.split_index)
    for (let i = 0; i < sorted.length; i++) {
      const segment = i === 0
        ? sorted[i].split_time
        : sorted[i].split_time - sorted[i - 1].split_time
      const cur = bestSegments.get(sorted[i].split_index)
      if (cur === undefined || segment < cur) bestSegments.set(sorted[i].split_index, segment)
    }
  }
  if (bestSegments.size === 0) return null
  let total = 0
  for (const v of bestSegments.values()) total += v
  return total
}

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

function askDelete(id: number) {
  pendingDeleteId.value = id
}

function cancelDelete() {
  pendingDeleteId.value = null
}

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
const text = getCssVar('--text-color')

const chartHeight = ref(247)
const yTicksLimit = computed(() => Math.max(2, Math.floor(chartHeight.value / 20)))

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
      const datasetSplitIndex = (dataset as any).splitIndex as number | undefined
      const split = datasetSplitIndex !== undefined
        ? sorted.find(s => s.split_index === datasetSplitIndex)
        : sorted.find(s => s.split_name === splitName || `Split ${s.split_index}` === splitName)
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

function buildChartData() {
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

  const datasets: any[] = []
  const totalData = visibleChartRuns.value.map(r => r.success ? r.total_time : null)
  const globalMinTime = Math.min(
    ...chartRuns.value
      .filter(r => r.success)
      .map(r => r.total_time)
  )
  const maxSplitIndex = Math.max(
    ...visibleChartRuns.value
      .filter(r => r.success)
      .flatMap(r => r.splits)
      .map(s => s.split_index),
    -Infinity
  )

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
      splitIndex: splitIndex,
      data: splitData,
      borderColor: 'rgba(125,125,125,0.5)',
      backgroundColor: 'transparent',
      pointBackgroundColor: pointColors,
      pointBorderColor: pointColors,
      pointRadius: 3,
      tension: 0.3,
    })
  })

  return { labels, datasets }
}

function buildChart() {
  if (!chartCanvas.value || visibleChartRuns.value.length === 0) return

  if (chartInstance) {
    const { labels, datasets } = buildChartData()
    chartInstance.data.labels = labels
    chartInstance.data.datasets = datasets
    chartInstance.options.scales!.y!.ticks!.maxTicksLimit = yTicksLimit.value
    chartInstance.update('none')
    return
  }

  const { labels, datasets } = buildChartData()
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
              return items[0].dataset.label ?? ''
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
    <div class="template-title" v-if="summary.template_name && runs.length >= 2">
      <span>{{ summary.template_name }}</span>
      <span v-if="runs.some(r => r.success) && getSumOfBest(runs) !== null">{{ $t('sum_of_best') }}: {{ formatRunTime(getSumOfBest(runs)!) }}</span>
      <span>{{ $t('failures') }}: {{ localAbortCount }}</span>
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
      <div class="segments-toggle">
        <label class="custom-checkbox" :class="{ 'checkbox-disabled': showSplits && !showSegments && !showGroupTotals }">
          {{ $t('splits') }}
          <input
            type="checkbox"
            v-model="showSplits"
            :disabled="showSplits && !showSegments && !showGroupTotals"
            @change="onShowSplitsChange"
          />
          <span class="checkmark"></span>
        </label>
      </div>
      <div class="segments-toggle">
        <label class="custom-checkbox" :class="{ 'checkbox-disabled': showSegments && !showSplits && !showGroupTotals }">
          {{ $t('show_segments') }}
          <input
            type="checkbox"
            v-model="showSegments"
            :disabled="showSegments && !showSplits && !showGroupTotals"
            @change="onShowSegmentsChange"
          />
          <span class="checkmark"></span>
        </label>
      </div>
      <div class="segments-toggle">
        <label class="custom-checkbox" :class="{ 'checkbox-disabled': showGroupTotals && !showSplits && !showSegments }">
          {{ $t('show_group_totals') }}
          <input
            type="checkbox"
            v-model="showGroupTotals"
            :disabled="showGroupTotals && !showSplits && !showSegments"
            @change="onShowGroupTotalsChange"
          />
          <span class="checkmark"></span>
        </label>
      </div>
    </div>

    <div class="history-list">
      <div v-if="loading" class="status-msg">{{ $t('loading') }}</div>
      <div v-else-if="runs.length === 0" class="status-msg">{{ $t('no_races') }}</div>

      <template v-else>
        <div
          v-for="group in processedGroups"
          :key="group.key"
          class="sequence-block"
        >
          <div class="history-table-wrap">
            <table class="history-table">
              <thead>
                <tr class="thead-splits">
                  <th
                    class="th-date sortable col-border-right"
                    :class="{ 'col-active': group.activeSortCol === 'date' }"
                    @click="toggleTableSort(group.key, 'date')"
                  >
                    {{ $t('date') }} <span class="sort-icon">{{ tableSortIcon(group.key, 'date', group.activeSortCol) }}</span>
                  </th>
                  <template v-for="block in group.groupedSplitColumns" :key="'grp-' + block.group_index">
                    <template v-for="(col, idx) in block.cols" :key="col.index">
                      <th
                        v-if="showSegments && !(block.group_index === 0 && idx === 0) && !(idx === 0)"
                        class="th-segment sortable col-border-right"
                        :class="{ 'col-active': group.activeSortCol === 'seg:' + block.cols[idx-1].index + ':' + col.index }"
                        @click="toggleTableSort(group.key, 'seg:' + block.cols[idx-1].index + ':' + col.index)"
                      >
                        <span class="seg-label">{{ block.cols[idx-1].name }} → {{ col.name }}</span>
                        <span class="sort-icon">{{ tableSortIcon(group.key, 'seg:' + block.cols[idx-1].index + ':' + col.index, group.activeSortCol) }}</span>
                      </th>
                      <th
                        v-if="(block.group_index > 0 || idx > 0) && showSplits"
                        class="th-split sortable col-border-right"
                        :class="{ 'col-active': group.activeSortCol === 'split:' + col.index }"
                        @click="toggleTableSort(group.key, 'split:' + col.index)"
                      >
                        {{ col.name }} <span class="sort-icon">{{ tableSortIcon(group.key, 'split:' + col.index, group.activeSortCol) }}</span>
                      </th>
                    </template>
                    <th v-if="showGroupTotals" class="th-group-total col-border-right">
                      {{ block.cols[0].name }}
                    </th>
                  </template>
                  <th class="th-del"></th>
                </tr>
              </thead>
              <tbody>
                <template v-for="row in group.processedRows" :key="row.run.id">
                  <tr v-if="pendingDeleteId === row.run.id" class="tr-confirm-delete">
                    <td class="td-date col-border-right">
                      <div class="rtime">{{ formatTimeOfDay(row.run.created_at) }}</div>
                      <div class="rdate">{{ formatDate(row.run.created_at) }}</div>
                    </td>
                    <td :colspan="9999" class="td-confirm-delete">
                      <span class="confirm-delete-text">{{ $t('delete_record') }}</span>
                      <button class="confirm-deletion-record" @click="confirmDelete(row.run.id)">{{ $t('delete') }}</button>
                      <button class="cancel-deletion-record" @click="cancelDelete()">{{ $t('cancel') }}</button>
                    </td>
                  </tr>
                  <tr v-else class="tr-splits" :class="{ 'tr-failed': !row.run.success }">
                    <td class="td-date col-border-right" :class="{ 'col-active-cell': group.activeSortCol === 'date' }">
                      <div class="rtime">{{ formatTimeOfDay(row.run.created_at) }}</div>
                      <div class="rdate">{{ formatDate(row.run.created_at) }}</div>
                    </td>
                    <template v-for="block in group.groupedSplitColumns" :key="'grp-' + block.group_index">
                      <template v-for="(col, idx) in block.cols" :key="col.index">
                        <td
                          v-if="showSegments && !(block.group_index === 0 && idx === 0) && !(idx === 0)"
                          class="td-segment col-border-right"
                          :class="{
                            'col-active-cell': group.activeSortCol === 'seg:' + block.cols[idx-1].index + ':' + col.index,
                            'best-split': row.cells.get(block.group_index + ':' + col.index)?.isBestSegment
                          }"
                        >
                          <span v-if="row.cells.get(block.group_index + ':' + col.index)?.segmentTime !== null">
                            {{ formatRunTime(row.cells.get(block.group_index + ':' + col.index)!.segmentTime!) }}
                          </span>
                          <span v-else class="no-split">—</span>
                        </td>
                        <td
                          v-if="(block.group_index > 0 || idx > 0) && showSplits"
                          class="td-split col-border-right"
                          :class="{
                            'best-split': row.cells.get(block.group_index + ':' + col.index)?.isBestSplit,
                            'col-active-cell': group.activeSortCol === 'split:' + col.index
                          }"
                        >
                          <template v-if="row.cells.get(block.group_index + ':' + col.index)?.splitTime !== null">
                            {{ formatRunTime(row.cells.get(block.group_index + ':' + col.index)!.splitTime!) }}
                          </template>
                          <template v-else-if="row.cells.get(block.group_index + ':' + col.index)?.isFail">
                            <span class="run-failed-cell">{{ $t('fail') }}</span>
                          </template>
                          <span v-else class="no-split">—</span>
                        </td>
                      </template>
                      <td v-if="showGroupTotals" class="td-group-total col-border-right">
                        <span v-if="row.cells.get(block.group_index + ':' + block.cols[0].index)?.groupTime !== null">
                          {{ formatRunTime(row.cells.get(block.group_index + ':' + block.cols[0].index)!.groupTime!) }}
                        </span>
                        <span v-else class="no-split">—</span>
                      </td>
                    </template>
                    <td class="td-del">
                      <button
                        class="record-delete"
                        :disabled="pendingDeleteId === row.run.id"
                        @click="askDelete(row.run.id)"
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
      </template>
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

.segments-toggle {
  display: flex;
  justify-self: center;
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
.status-msg{
  padding: 10px;
}

.sequence-block {
  border-bottom: 2px solid rgba(255, 0, 0, 0.5);
  /* width: max-content; */
  margin-bottom: 4px;
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

.th-del{
  background: linear-gradient(90deg, var(--card-bg) 50%, var(--bg-color) 100%) !important;
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
.tr-confirm-delete td {
  padding: 4px;
  text-align: center;
  vertical-align: middle;
  white-space: nowrap;
}
.tr-confirm-delete .td-confirm-delete {
  text-align: end;
}
.tr-splits:hover {
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
.checkbox-disabled {
  opacity: 0.4;
  cursor: not-allowed;
  pointer-events: none;
}

.th-group-total,
.td-group-total {
  text-align: center;
  white-space: nowrap;
}
.td-group-total{
  color: var(--text-group-color);
}
</style>