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
import imgVisibility from '../../imgs/visibility.png'

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
  visibility: number
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

const deletionMode = ref(false)
const selectedRunIds = ref<Set<number>>(new Set())
const rangeFrom = ref('')
const rangeTo = ref('')

const isDeleting = ref(false)

const confirmDialog = ref<{ visible: boolean; label: string; ids: number[] }>({
  visible: false,
  label: '',
  ids: [],
})

function askBatchDelete(label: string, ids: number[]) {
  if (isDeleting.value || ids.length === 0) return
  confirmDialog.value = { visible: true, label, ids }
}

function cancelConfirm() {
  if (isDeleting.value) return
  confirmDialog.value.visible = false
}

async function confirmBatchDelete() {
  const ids = confirmDialog.value.ids
  await deleteBatch(ids)
}

async function toggleDeletionMode() {
  if (isDeleting.value) return
  deletionMode.value = !deletionMode.value
  if (!deletionMode.value) {
    selectedRunIds.value = new Set()
    rangeFrom.value = ''
    rangeTo.value = ''
  }
}

function toggleSelectRun(id: number) {
  if (isDeleting.value) return
  const s = new Set(selectedRunIds.value)
  if (s.has(id)) s.delete(id)
  else s.add(id)
  selectedRunIds.value = s
}

const failureRuns = computed(() => runs.value.filter(r => !r.success))

const rangeFilteredRuns = computed(() => {
  if (!rangeFrom.value && !rangeTo.value) return []
  const from = rangeFrom.value ? new Date(rangeFrom.value).getTime() / 1000 : 0
  const to = rangeTo.value ? new Date(rangeTo.value).getTime() / 1000 : Infinity
  return runs.value.filter(r => r.created_at >= from && r.created_at <= to)
})

async function deleteBatch(idsToDelete: number[]) {
  if (isDeleting.value || idsToDelete.length === 0) return
  isDeleting.value = true
  try {
    const idsSet = new Set(idsToDelete)
    const runsToDelete = runs.value.filter(r => idsSet.has(r.id))
    const failCount = runsToDelete.filter(r => !r.success).length

    await Promise.all(idsToDelete.map(id => invoke('delete_run', { runId: id })))

    runs.value = runs.value.filter(r => !idsSet.has(r.id))

    if (failCount > 0) {
      localAbortCount.value = Math.max(0, localAbortCount.value - failCount)
      await tauriEmit('abort-decremented', {})
    }

    confirmDialog.value.visible = false
    selectedRunIds.value = new Set()
    rangeFrom.value = ''
    rangeTo.value = ''

    if (runs.value.length === 0) {
      deletionMode.value = false
      emit('deleted')
      emit('close')
    } else {
      deletionMode.value = false
      emit('deleted')
    }
  } catch (e) {
    console.error(e)
  } finally {
    isDeleting.value = false
  }
}

async function deleteSelected() {
  askBatchDelete(t('delete_selected') + ` (${selectedRunIds.value.size})`, [...selectedRunIds.value])
}

async function deleteFailures() {
  askBatchDelete(t('delete_failed') + ` (${failureRuns.value.length})`, failureRuns.value.map(r => r.id))
}

async function deleteInRange() {
  askBatchDelete(t('delete') + ` (${rangeFilteredRuns.value.length})`, rangeFilteredRuns.value.map(r => r.id))
}

async function deleteAll() {
  askBatchDelete(t('delete_all') + ` (${runs.value.length})`, runs.value.map(r => r.id))
}

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

async function toggleVisibility(run: Run) {
  const newVisibility = run.visibility === 1 ? 0 : 1
  try {
    await invoke('set_run_visibility', { runId: run.id, visibility: newVisibility })
    run.visibility = newVisibility
  } catch (e) {
    console.error(e)
  }
}

function onWindowResize() { chartWidth.value = window.innerWidth }

onMounted(async () => {
  window.addEventListener('keydown', handleKeyDown)
  window.addEventListener('resize', onWindowResize)

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
  [...runs.value].filter(r => r.visibility !== 0).sort((a, b) => a.created_at - b.created_at)
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
const chartWidth = ref(window.innerWidth)

const yTicksLimit = computed(() => Math.max(2, Math.floor(chartHeight.value / 20)))

const pointStep = computed(() => {
  const n = visibleChartRuns.value.length
  if (n <= 1) return 1
  const pxPerLabel = (chartWidth.value * 0.99) / n
  if (pxPerLabel >= 20) return 1
  if (pxPerLabel >= 10) return 2
  if (pxPerLabel >= 5)  return 4
  if (pxPerLabel >= 2)  return 10
  return Math.ceil(20 / Math.max(pxPerLabel, 0.5))
})

const sampledChartRuns = computed(() => {
  const step = pointStep.value
  if (step <= 1) return visibleChartRuns.value
  return visibleChartRuns.value.filter((_, i) => i % step === 0)
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

    const run = sampledChartRuns.value[closestRunIndex]
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
  const labels = sampledChartRuns.value.map(r => {
    const globalIndex = chartRuns.value.findIndex(cr => cr.id === r.id)
    return `${globalIndex + 1}`
  })

  const splitIndexes = new Set<number>()
  sampledChartRuns.value.forEach(run => {
    run.splits.forEach(s => {
      if (s.split_index !== 0) splitIndexes.add(s.split_index)
    })
  })

  const datasets: any[] = []
  const totalData = sampledChartRuns.value.map(r => r.success ? r.total_time : null)
  const globalMinTime = Math.min(
    ...chartRuns.value
      .filter(r => r.success)
      .map(r => r.total_time)
  )
  const maxSplitIndex = Math.max(
    ...sampledChartRuns.value
      .filter(r => r.success)
      .flatMap(r => r.splits)
      .map(s => s.split_index),
    -Infinity
  )

  const lastSplitName = sampledChartRuns.value
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
      sampledChartRuns.value.find(r => r.splits.find(s => s.split_index === splitIndex))
        ?.splits.find(s => s.split_index === splitIndex)?.split_name || `Split ${splitIndex}`

    const splitData = sampledChartRuns.value.map(run => {
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
  if (!chartCanvas.value || sampledChartRuns.value.length === 0) return

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
          filter(item) {
            return item.dataIndex % pointStep.value === 0
          },
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

watch(deletionMode, (val) => {
  if (!val) nextTick(() => buildChart())
})

watch(sampledChartRuns, async () => {
  await nextTick()
  buildChart()
})

watch(yTicksLimit, () => {
  buildChart()
})

watch(pointStep, () => {
  buildChart()
})

onUnmounted(() => {
  if (unlistenRunSaved) unlistenRunSaved()
  if (chartInstance) {
    chartInstance.destroy()
    chartInstance = null
  }
  window.removeEventListener('resize', onWindowResize)
})
</script>

<template>
  <div class="race-statistic">
    <div class="template-title" v-if="summary.template_name && runs.length >= 2">
      <span>{{ summary.template_name }}</span>
      <span v-if="!deletionMode && runs.some(r => r.success && r.visibility !== 0) && getSumOfBest(runs.filter(r => r.visibility !== 0)) !== null">{{ $t('sum_of_best') }}: {{ formatRunTime(getSumOfBest(runs.filter(r => r.visibility !== 0))!) }}</span>
      <span v-if="!deletionMode && localAbortCount > 0">{{ $t('failures') }}: {{ localAbortCount }}</span>
    </div>

    <div
      class="chart-container"
      v-if="runs.length >= 2"
      v-show="!deletionMode"
      :style="{ height: chartHeight + 'px' }"
      @wheel.prevent="onChartWheel"
    >
      <canvas ref="chartCanvas"></canvas>
    </div>

    <div class="chart-controls" v-if="chartRuns.length > CHART_MIN_VISIBLE" v-show="!deletionMode">
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
      v-show="!deletionMode"
    >
      <span class="chart-resizer-dots">• • •</span>
    </div>

    <div v-show="deletionMode" class="deletion-container">
      <div class="deletion-range">
        <div class="deletion-range-desc">{{ $t('delete_in_range') }}:</div>
        <div class="deletion-range-from">{{ $t('from') }}
          <input type="datetime-local" v-model="rangeFrom">
        </div>
        <div class="deletion-range-to">{{ $t('to') }}
          <input type="datetime-local" v-model="rangeTo">
        </div>
        <button class="deletion-range-button" :disabled="isDeleting || rangeFilteredRuns.length === 0" @click="deleteInRange">{{$t('delete')}} ({{ rangeFilteredRuns.length }})</button>
      </div>
      <div class="deletion-variants">
        <div class="deleting-all">
          <button :disabled="isDeleting" @click="deleteAll">{{ $t('delete_all') }} ({{ runs.length }})</button>
        </div>
        <div class="deleting-failures">
          <button :disabled="isDeleting || failureRuns.length === 0" @click="deleteFailures">{{ $t('delete_failed') }} ({{ failureRuns.length }})</button>
        </div>
        <div class="deleting-selected">
          <button :disabled="isDeleting || selectedRunIds.size === 0" @click="deleteSelected">{{ $t('delete_selected') }} ({{ selectedRunIds.size }})</button>
        </div>
      </div>
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
      <div class="deletion-records">
        <button :disabled="isDeleting" @click="toggleDeletionMode">{{ deletionMode ? $t('cancel') : $t('delete_records') }}</button>
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
                        v-if="showSegments && !(block.group_index === 0 && idx === 0) && !(idx === 1)"
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
                  <th v-if="deletionMode" class="th-checkbox"></th>
                  <th v-if="!deletionMode" class="th-visibility"></th>
                </tr>
              </thead>
              <tbody>
                <template v-for="row in group.processedRows" :key="row.run.id">
                  <tr
                    class="tr-splits"
                    :class="{ 'tr-failed': !row.run.success, 'tr-selected': deletionMode && selectedRunIds.has(row.run.id), 'tr-hidden': row.run.visibility === 0 }"
                    @click="deletionMode && toggleSelectRun(row.run.id)"
                  >
                    <td class="td-date col-border-right" :class="{ 'col-active-cell': group.activeSortCol === 'date' }">
                      <div class="rtime">{{ formatTimeOfDay(row.run.created_at) }}</div>
                      <div class="rdate">{{ formatDate(row.run.created_at) }}</div>
                    </td>
                    <template v-for="block in group.groupedSplitColumns" :key="'grp-' + block.group_index">
                      <template v-for="(col, idx) in block.cols" :key="col.index">
                        <td
                          v-if="showSegments && !(block.group_index === 0 && idx === 0) && !(idx === 1)"
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
                    <td v-if="deletionMode" class="td-checkbox">
                      <label class="custom-checkbox" @click.stop>
                        <input
                          type="checkbox"
                          :checked="selectedRunIds.has(row.run.id)"
                          @change="toggleSelectRun(row.run.id)"
                          @click.stop
                        />
                        <span class="checkmark" @click.stop></span>
                      </label>
                    </td>
                    <td v-if="!deletionMode" class="td-visibility" :title="row.run.visibility === 0 ? $t('show') : $t('hide')" @click.stop="toggleVisibility(row.run)">
                      <span class="visibility-icon" :class="{ 'visibility-icon--hidden': row.run.visibility === 0 }">
                        <img :src="imgVisibility"/>
                      </span>
                    </td>
                  </tr>
                </template>
              </tbody>
            </table>
          </div>
        </div>
      </template>
    </div>

    <Transition name="confirm-fade">
      <div
        v-if="confirmDialog.visible"
        class="confirm-overlay"
        @click.self="cancelConfirm"
      >
        <div class="confirm-dialog">
          <p class="confirm-name">{{ summary.template_name }}</p>
          <p class="confirm-text">{{ confirmDialog.label }} {{ $t('records') }}?</p>
          <div class="confirm-actions">
            <button class="button button--danger" :disabled="isDeleting" @click="confirmBatchDelete">
              <template v-if="isDeleting">{{ $t('in_progress') }}...</template>
              <template v-else>{{ $t('delete') }}</template>
            </button>
            <button v-show="!isDeleting" class="button" @click="cancelConfirm">
              {{ $t('cancel') }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
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

.deletion-records{
  width: 100%;
  display: flex;
  justify-content: end;
  padding-right: 10px;
}
.deletion-records > button{
  height: 100%;
}

.template-title {
  display: flex;
  flex-direction: row;
  margin: 5px 5px -10px 5px;
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

.history-table-wrap {
  min-width: max-content;
  border-bottom: 2px solid rgba(255, 0, 0, 0.5);
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

.tr-splits td{
  padding: 4px;
  text-align: center;
  vertical-align: middle;
  white-space: nowrap;
}
.tr-splits:hover {
  background-color: var(--card-bg);
}
.tr-selected td {
  background: rgba(255, 80, 80, 0.15) !important;
}
.tr-splits:has(.td-checkbox) {
  cursor: pointer;
}
.tr-splits:has(.td-checkbox):hover td {
  background-color: rgba(255, 80, 80, 0.25) !important;
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

.checkbox-disabled {
  opacity: 0.4;
  cursor: not-allowed;
  pointer-events: none;
}

.deletion-container{
  margin-top: 20px;
  box-shadow: 3px 3px 0 0 rgba(0,0,0,0.5), 3px -3px 0 0 rgba(0,0,0,0.5);
}
.deletion-container,
.deletion-container > div{
  width: 100%;
  min-height: 160px;
  display: flex;
  flex-direction: row;
}
.deletion-container > div{
  flex-direction: column;
  align-items: center;
  justify-content: center;
}
.deletion-range{
  box-shadow: 3px 0px 0 0 rgba(0,0,0,0.5);
}
.deletion-range-from{
  margin-top: 10px;
}
.deletion-range-to{
  margin: 5px 0 10px 0;
}
.deletion-range-button{
  height: 30px;
  padding: 0 10px;
}
.deletion-variants{
  gap: 20px;
}
.deleting-all > button,
.deleting-failures > button,
.deleting-selected > button{
  height: 30px;
  padding: 0px 10px;
}
.th-checkbox{
  background: linear-gradient(90deg, var(--card-bg) 20%, var(--bg-color) 100%) !important;
}
.th-checkbox,
.td-checkbox {
  width: 36px;
  text-align: center;
  vertical-align: middle;
  padding: 4px;
}

.th-group-total,
.td-group-total {
  text-align: center;
  white-space: nowrap;
}
.td-group-total{
  color: var(--text-group-color);
}
.confirm-overlay {
  position: absolute;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(3px);
  -webkit-backdrop-filter: blur(3px);
}

.confirm-dialog {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  width: max-content;
  padding: 10px;
  background: var(--stngs-bg-color);
  box-shadow: 0 0 100px 50px rgba(0, 0, 0, 0.7);
}

.confirm-text,
.confirm-name {
  margin: 0;
}

.confirm-actions {
  display: flex;
  flex-direction: row;
  gap: 20px;
  margin-top: 6px;
}

.confirm-actions > button {
  height: 30px;
}

.confirm-fade-enter-active,
.confirm-fade-leave-active {
  transition: opacity 0.15s ease;
}
.confirm-fade-enter-from,
.confirm-fade-leave-to {
  opacity: 0;
}

.th-visibility,
.td-visibility {
  width: 35px;
}

.td-visibility {
  cursor: pointer;
}

.visibility-icon {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
}
.visibility-icon img{
  width: 25px;
  height: 25px;
}

.visibility-icon--hidden img {
  opacity: 0.4;
}

.visibility-icon--hidden::before,
.visibility-icon--hidden::after {
  content: "";
  position: absolute;
  left: 50%;
  top: 50%;
  width: 100%;
  height: 1.5px;
  background: currentColor;
  transform-origin: center;
}
.visibility-icon--hidden::before {
  transform: translate(-50%, -50%) rotate(45deg);
}
.visibility-icon--hidden::after {
  transform: translate(-50%, -50%) rotate(-45deg);
}

.tr-hidden td {
  opacity: 0.2;
}
</style>