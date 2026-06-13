<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch, nextTick, computed } from "vue"
import { settings } from "../services/settings"
import { WebviewWindow } from "@tauri-apps/api/webviewWindow"
import { LogicalPosition, LogicalSize } from "@tauri-apps/api/window"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import {
  overlayState,
  initOverlayListener,
  timerStartTime,
  firstSplitReceived,
  setStartTimerCallback,
  setStopTimerCallback,
  setPauseTimerCallback,
  setResumeTimerCallback
} from "../services/overlay"
import { useI18n } from 'vue-i18n'
import i18n from '../i18n'
import { formatShortcutDisplay } from '../composables/useKeybindContext'


const { t } = useI18n()

const overlayWindowRef = ref<WebviewWindow | null>(null)
const loaded = ref(false)
const overlayElement = ref<HTMLElement | null>(null)
const abortCount = ref(0)

const overlayBackground = computed(() => {
  const alpha = 1 - Math.min(1, Math.max(0, settings.overlay.overlay_transparent / 100))
  const clampedAlpha = Math.min(1, Math.max(0, alpha))
  return `rgba(0, 0, 0, ${clampedAlpha})`
})

interface BestSplit {
  split_index: number
  split_time: number
}

const bestSplitMap = ref<Map<number, number>>(new Map())
const goldSplitMap = ref<Map<number, number>>(new Map())
const bestRunTotalTime = ref<number | null>(null)
const hasBestRun = computed(() => bestSplitMap.value.size > 0)
const lastRuns = ref<Array<{ id: number, total_time: number }>>([])
const bestSegmentMap = ref<Map<number, number>>(new Map())
const frozenDeltaMap = ref<Map<number, number>>(new Map())
const frozenGoldSplitMap = ref<Map<number, boolean>>(new Map())
const frozenGoldSegmentMap = ref<Map<number, boolean>>(new Map())
const hasFrozenDeltas = computed(() => frozenDeltaMap.value.size > 0)

const disruptionPredictedTime = ref<number | null>(null)

const showPredictingTimer = computed(() =>
  (settings.overlay?.predicting_timer ?? false) &&
  overlayState.value.is_running &&
  disruptionPredictedTime.value !== null
)

function resetDisruptionPrediction() {
  disruptionPredictedTime.value = null
}

async function loadBestSegments(templateId: string) {
  if (!templateId) {
    bestSegmentMap.value = new Map()
    return
  }
  try {
    const segments = await invoke<Array<{ split_index: number, split_time: number }>>(
      'get_best_segments', { templateId }
    )
    const map = new Map<number, number>()
    for (const s of segments) map.set(s.split_index, s.split_time)
    bestSegmentMap.value = map
  } catch {
    bestSegmentMap.value = new Map()
  }
}

async function loadLastRuns(templateId: string) {
  if (!templateId) {
    lastRuns.value = []
    return
  }
  const count = settings.overlay?.sum_of_the_last ?? 0
  if (count <= 0) {
    lastRuns.value = []
    return
  }
  try {
    const runs = await invoke<Array<{ id: number, total_time: number, success: boolean, visibility: number }>>(
      'get_runs', { templateId }
    )
    lastRuns.value = runs.filter(r => r.success && r.visibility !== 0).slice(0, count)
  } catch {
    lastRuns.value = []
  }
}

const sumLast = computed((): number | null => {
  if (lastRuns.value.length === 0) return null
  return lastRuns.value.reduce((acc, r) => acc + r.total_time, 0)
})

const showSumLast = computed(() =>
  (settings.overlay?.sum_of_the_last ?? 0) > 1 &&
  lastRuns.value.length > 0
)

const sumOfBest = computed((): number | null => {
  if (bestSegmentMap.value.size === 0) return null
  let total = 0
  for (const duration of bestSegmentMap.value.values()) {
    total += duration
  }
  return total
})

const isRunActive = computed(() => overlayState.value.is_running)

async function loadBestRun(templateId: string) {
  if (!templateId) {
    bestSplitMap.value = new Map()
    bestRunTotalTime.value = null
    return
  }
  try {
    const run = await invoke<{
      splits: BestSplit[]
      total_time: number
    } | null>('get_best_run', { templateId })

    if (!run) {
      bestSplitMap.value = new Map()
      bestRunTotalTime.value = null
      return
    }

    const map = new Map<number, number>()
    for (const s of run.splits) {
      map.set(s.split_index, s.split_time)
    }
    bestSplitMap.value = map
    bestRunTotalTime.value = run.total_time
  } catch {
    bestSplitMap.value = new Map()
    bestRunTotalTime.value = null
  }
}
async function loadGoldSplits(templateId: string) {
  if (!templateId) {
    goldSplitMap.value = new Map()
    return
  }
  try {
    const splits = await invoke<Array<{ split_index: number, split_time: number }>>(
      'get_best_splits', { templateId }
    )
    const map = new Map<number, number>()
    for (const s of splits) map.set(s.split_index, s.split_time)
    goldSplitMap.value = map
  } catch {
    goldSplitMap.value = new Map()
  }
}

const displayTimer = ref("-:-")
let timerInterval: number | null = null
let timerFrozenTime = 0
let timerPaused = false

const runTitle = computed(() => {
  if (!overlayState.value.template_name) return t('waiting')
  return overlayState.value.template_name
})

function formatTime(sec: number): string {
  const totalMs = Math.floor(sec * 1000)
  const ms = totalMs % 1000
  const totalS = Math.floor(totalMs / 1000)
  const s = totalS % 60
  const totalM = Math.floor(totalS / 60)
  const m = totalM % 60
  const h = Math.floor(totalM / 60)

  let timePart = ""
  if (h > 0) {
  timePart = `${h}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`
  } else if (m > 0) {
    timePart = `${m}:${s.toString().padStart(2, "0")}`
  } else {
    timePart = `${s}`
  }

  const accuracy = settings.overlay?.time_accuracy || "milliseconds"

  switch (accuracy) {
    case "seconds":
      return timePart
    case "tenths": {
      const tenths = Math.floor(ms / 100)
      return `${timePart}.${tenths}`
    }
    case "hundredths": {
      const hundredths = Math.floor(ms / 10)
      return `${timePart}.${hundredths.toString().padStart(2, "0")}`
    }
    case "milliseconds":
    default:
      return `${timePart}.${ms.toString().padStart(3, "0")}`
  }
}

function formatDelta(delta: number): string {
  const sign = delta >= 0 ? '+' : '-'
  const abs = Math.abs(delta)

  const totalMs = Math.floor(abs * 1000)
  const ms = totalMs % 1000
  const totalS = Math.floor(totalMs / 1000)
  const s = totalS % 60
  const totalM = Math.floor(totalS / 60)
  const m = totalM % 60
  const h = Math.floor(totalM / 60)

  let timePart = ""
  if (h > 0) {
    timePart = s > 0
      ? `${h}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`
      : `${h}:${m.toString().padStart(2, "0")}`
  } else if (m > 0) {
    timePart = s > 0
      ? `${m}:${s.toString().padStart(2, "0")}`
      : `${m}`
  } else {
    timePart = `${s}`
  }

  const accuracy = settings.overlay?.time_accuracy || "milliseconds"

  switch (accuracy) {
    case "seconds":
      return `${sign}${timePart}`
    case "tenths": {
      const tenths = Math.floor(ms / 100)
      return `${sign}${timePart}.${tenths}`
    }
    case "hundredths": {
      const hundredths = Math.floor(ms / 10)
      return `${sign}${timePart}.${hundredths.toString().padStart(2, "0")}`
    }
    case "milliseconds":
    default:
      return `${sign}${timePart}.${ms.toString().padStart(3, "0")}`
  }
}

function startFakeTimer() {
  if (timerInterval) clearInterval(timerInterval)
  timerStartTime.value = Date.now()
  timerFrozenTime = 0
  timerPaused = false
  displayTimer.value = "0.000"

  timerInterval = window.setInterval(() => {
    if (timerPaused) return
    const elapsed = timerFrozenTime + (Date.now() - timerStartTime.value) / 1000
    displayTimer.value = formatTime(elapsed)
  }, 16)
}

function pauseFakeTimer(frozenTime: number) {
  if (!timerInterval) return
  timerPaused = true
  timerFrozenTime = frozenTime
  displayTimer.value = formatTime(frozenTime)
}

function resumeFakeTimer() {
  if (!timerInterval) return
  timerStartTime.value = Date.now()
  timerPaused = false
}

function stopFakeTimer(finalTime: number | null) {
  if (timerInterval) {
    clearInterval(timerInterval)
    timerInterval = null
  }
  timerPaused = false
  timerFrozenTime = 0
  if (finalTime !== null) {
    displayTimer.value = formatTime(finalTime)
  }
}

function getLiveDelta(order: number, currentTime: number | null): number | null {
  if (!hasBestRun.value || currentTime === null) return null
  const bestTime = bestSplitMap.value.get(order)
  if (bestTime === undefined) return null
  return currentTime - bestTime
}

function getBestSplitTime(order: number): number | null {
  if (!hasBestRun.value) return null
  return bestSplitMap.value.get(order) ?? null
}

function getDisplayDelta(order: number, currentTime: number | null): number | null {
  if (isRunActive.value) {
    return getLiveDelta(order, currentTime)
  }
  if (hasFrozenDeltas.value) {
    const frozen = frozenDeltaMap.value.get(order)
    return frozen !== undefined ? frozen : null
  }
  return null
}

function freezeCurrentDeltas() {
  const deltaMap = new Map<number, number>()
  const goldSplitResult = new Map<number, boolean>()
  const goldSegmentResult = new Map<number, boolean>()
  for (const split of overlayState.value.splits) {
    if (!split.is_completed || split.split_time === null) continue
    const delta = getLiveDelta(split.order, split.split_time)
    if (delta !== null) {
      deltaMap.set(split.order, delta)
      goldSplitResult.set(split.order, isGoldSplit(split.order, split.split_time))
      goldSegmentResult.set(split.order, isGoldSegment(split.order, split.split_time))
    }
  }
  frozenDeltaMap.value = deltaMap
  frozenGoldSplitMap.value = goldSplitResult
  frozenGoldSegmentMap.value = goldSegmentResult
}
function getFrozenGold(order: number): boolean {
  const mode = settings.overlay?.time_gold ?? 'splits'
  const map = mode === 'segments' ? frozenGoldSegmentMap.value : frozenGoldSplitMap.value
  return map.get(order) ?? false
}

function isGoldSplit(order: number, currentTime: number | null): boolean {
  if (currentTime === null) return false
  const gold = goldSplitMap.value.get(order)
  if (gold === undefined) return false
  return currentTime < gold
}

function isGoldSegment(order: number, currentTime: number | null): boolean {
  if (currentTime === null) return false
  const bestSegment = bestSegmentMap.value.get(order)
  if (bestSegment === undefined) return false

  const splits = overlayState.value.splits
  const prevSplit = splits.find(s => s.order === order - 1)
  const prevTime = (prevSplit?.is_completed && prevSplit.split_time !== null)
    ? prevSplit.split_time
    : 0
  const segmentDuration = currentTime - prevTime

  return segmentDuration < bestSegment
}

function isGold(order: number, currentTime: number | null): boolean {
  const mode = settings.overlay?.time_gold ?? 'splits'
  return mode === 'segments'
    ? isGoldSegment(order, currentTime)
    : isGoldSplit(order, currentTime)
}

watch(() => overlayState.value.is_trigger_only, async (isTriggerOnly) => {
  await nextTick()
  if (isTriggerOnly && overlayState.value.is_running && !timerInterval) {
    startFakeTimer()
  }
})

watch(() => overlayState.value.is_running, (isRunning) => {
  if (isRunning) {
    frozenDeltaMap.value = new Map()
    frozenGoldSplitMap.value = new Map()
    frozenGoldSegmentMap.value = new Map()
  } else {
    resetDisruptionPrediction()
  }
})

watch(firstSplitReceived, async (received) => {
  await nextTick()
  if (received && overlayState.value.is_running && !timerInterval) {
    startFakeTimer()
    firstSplitReceived.value = false
  }
})

watch(
  () => overlayState.value.template_id,
  async (templateId) => {
    if (!templateId) {
      abortCount.value = 0
      bestSplitMap.value = new Map()
      bestRunTotalTime.value = null
      frozenDeltaMap.value = new Map()
      frozenGoldSplitMap.value = new Map()
      frozenGoldSegmentMap.value = new Map()
      return
    }

    try {
      const summaries = await invoke<Array<{
        template_id: string
        abort_count: number
      }>>('get_template_summaries')
      const match = summaries.find(
        s => s.template_id === templateId
      )
      abortCount.value = match?.abort_count ?? 0
    } catch {
      abortCount.value = 0
    }

    await loadBestRun(templateId)
    await loadGoldSplits(templateId)
    await loadBestSegments(templateId)
    await loadLastRuns(templateId)
  }
)

watch(
  () => settings.overlay?.sum_of_the_last,
  async (val) => {
    const { template_id } = overlayState.value
    if (!template_id) return
    if (!val || val <= 1) {
      lastRuns.value = []
      return
    }
    await loadLastRuns(template_id)
  }
)

let isDragging = ref(false)
let handleGlobalMouseUp: (() => void) | null = null
let resizeObserver: ResizeObserver | null = null

const onMouseDown = async () => {
  if (!overlayWindowRef.value || !settings.overlay?.drag_mode) return
  isDragging.value = true
  await overlayWindowRef.value.startDragging()
}

const releaseCursor = async () => {
  if (!overlayWindowRef.value) return
  try {
    await overlayWindowRef.value.setCursorGrab(false)
  } catch {}
  isDragging.value = false
}

const savePosition = async () => {
  if (!overlayWindowRef.value) return
  const pos = await overlayWindowRef.value.outerPosition()
  settings.overlay.pos_x = pos.x
  settings.overlay.pos_y = pos.y
  await invoke("set_settings", { newSettings: { ...settings } })
}

const setPosition = async (targetX: number, targetY: number) => {
  if (!overlayWindowRef.value) return
  await overlayWindowRef.value.setPosition(new LogicalPosition(targetX, targetY))
}

const updateModeStyles = async () => {
  try {
  if (settings.overlay.drag_mode) {
      await overlayWindowRef.value?.setIgnoreCursorEvents(false)
    } else {
      await overlayWindowRef.value?.setIgnoreCursorEvents(true)
    }
  } catch {}
}

const visibleSplits = computed(() => overlayState.value.splits.slice(1))

const showRunTitle = computed(() => {
  if (!overlayState.value.template_name) return true
  return settings.overlay?.run_name ?? true
})
const showSumOfBest = computed(() =>
  (settings.overlay?.sum_of_best ?? true) && overlayState.value.template_name !== ""
)
const showSplitSeparators = computed(() =>
  settings.overlay?.split_separators ?? true
)
const paginatedSplits = computed(() => {
  const splits = visibleSplits.value
  if (splits.length === 0) return splits

  const limit = settings.overlay?.number_of_splits ?? 3
  if (limit <= 0) return splits

  const activeIndex = splits.findIndex(split => !split.is_completed)
  if (activeIndex === -1) return splits.slice(-limit)

  let start = Math.max(0, activeIndex - Math.floor(limit / 2))
  if (start + limit > splits.length) {
    start = Math.max(0, splits.length - limit)
  }
  return splits.slice(start, start + limit)
})
const showSplits = computed(() =>
  (settings.overlay?.show_splits ?? true) && overlayState.value.template_name !== ""
)
const showGroupList = computed(() =>
  (settings.overlay?.group_list ?? true) &&
  overlayState.value.is_running &&
  overlayState.value.pending_groups.length > 0
)
const showRunAborted = computed(() =>
  (settings.overlay?.run_aborted ?? true) && overlayState.value.template_name !== ""
)
const showTimer = computed(() => {
  const showBySetting = settings.overlay?.fake_timer ?? true
  const hasTimerValue = overlayState.value.current_timer !== null
  return showBySetting && (overlayState.value.is_running || hasTimerValue)
})

const showComparisonColumns = computed(() =>
  hasBestRun.value && (isRunActive.value || hasFrozenDeltas.value)
)

watch(
  () => settings.overlay?.drag_mode,
  async () => {
    if (!settings.overlay) return
    updateModeStyles()
    if (overlayWindowRef.value && !isDragging.value) {
      try {
        await overlayWindowRef.value.setCursorGrab(false)
      } catch {}
    }
  }
)

onMounted(async () => {
  await listen<string>('language-changed', (event) => {
    i18n.global.locale.value = event.payload as any
  })

  initOverlayListener()

  await listen('run-saved', async () => {
    const { template_id } = overlayState.value
    if (!template_id) return

    freezeCurrentDeltas()

    await loadBestRun(template_id)
    await loadGoldSplits(template_id)
    await loadBestSegments(template_id)
    await loadLastRuns(template_id)

    try {
      const summaries = await invoke<Array<{
        template_id: string
        abort_count: number
      }>>('get_template_summaries')
      const match = summaries.find(s => s.template_id === template_id)
      abortCount.value = match?.abort_count ?? 0
    } catch {}
  })

  await listen('run-reset', () => {
    stopFakeTimer(null)
    resetDisruptionPrediction()
    overlayState.value = {
      ...overlayState.value,
      is_running: false,
    }
  })

  await listen('abort-incremented', async () => {
    const { template_id } = overlayState.value
    if (!template_id) return
    try {
      const summaries = await invoke<Array<{
        template_id: string
        abort_count: number
      }>>('get_template_summaries')
      const match = summaries.find(s => s.template_id === template_id)
      abortCount.value = match?.abort_count ?? 0
    } catch {}
  })
  await listen('abort-decremented', async () => {
    const { template_id } = overlayState.value
    if (!template_id) return
    try {
      const summaries = await invoke<Array<{
        template_id: string
        abort_count: number
      }>>('get_template_summaries')
      const match = summaries.find(s => s.template_id === template_id)
      abortCount.value = match?.abort_count ?? 0
    } catch {}
  })

  await listen('run-visibility-changed', async () => {
    const template_id = overlayState.value.template_id
    if (!template_id) return
    await loadBestRun(template_id)
    await loadGoldSplits(template_id)
    await loadBestSegments(template_id)
    await loadLastRuns(template_id)
  })

  await listen<{ predicted_time: number | null; completed_rounds: number }>(
    'disruption-prediction',
    (event) => {
      disruptionPredictedTime.value = event.payload.predicted_time
    }
  )

  setStartTimerCallback(() => {
    if (!timerInterval) startFakeTimer()
  })
  setStopTimerCallback((finalTime: number | null) => {
    stopFakeTimer(finalTime)
  })
  setPauseTimerCallback((frozenTime: number) => {
    pauseFakeTimer(frozenTime)
  })
  setResumeTimerCallback(() => {
    resumeFakeTimer()
  })

  overlayWindowRef.value = await WebviewWindow.getByLabel("overlay-window")
  if (!overlayWindowRef.value) return

  await listen("toggle-overlay-mode", () => {
    if (settings.overlay) {
      settings.overlay.drag_mode = !settings.overlay.drag_mode
      invoke("set_settings", { newSettings: { ...settings } })
    }
  })

  overlayElement.value = document.querySelector('.overlay-window') as HTMLElement

  if (settings.overlay?.show) await overlayWindowRef.value.show()

  await new Promise(r => setTimeout(r, 50))
  await updateSize()

  if (settings.overlay) {
    await setPosition(settings.overlay.pos_x, settings.overlay.pos_y)
  }

  updateModeStyles()

  await overlayWindowRef.value.onMoved(async () => {
    await savePosition()
    if (isDragging.value) await releaseCursor()
  })

  handleGlobalMouseUp = () => {
    if (isDragging.value) releaseCursor()
  }
  window.addEventListener('mouseup', handleGlobalMouseUp)

  if (overlayElement.value) {
    resizeObserver = new ResizeObserver(async () => {
      await updateSize()
    })
    resizeObserver.observe(overlayElement.value)
  }

  loaded.value = true
})

onUnmounted(() => {
  if (timerInterval) clearInterval(timerInterval)
  if (handleGlobalMouseUp) {
    window.removeEventListener('mouseup', handleGlobalMouseUp)
  }
  if (isDragging.value) releaseCursor()
  if (resizeObserver) {
    resizeObserver.disconnect()
    resizeObserver = null
  }
})

const updateSize = async () => {
  await nextTick()
  setTimeout(async () => { await updateSize() }, 100)
  if (!overlayElement.value || !overlayWindowRef.value) return

  const rect = overlayElement.value.getBoundingClientRect()
  const width = Math.min(Math.max(Math.ceil(rect.width), 50), 600)
  const height = Math.max(Math.ceil(rect.height), 20)

  try {
    await overlayWindowRef.value.setSize(new LogicalSize(width, height))
  } catch {}
}
</script>

<template>
  <div
    v-if="loaded"
    ref="overlayElement"
    class="overlay-window"
    :class="{ 'drag-mode': settings.overlay.drag_mode }"
    :style="{ backgroundColor: overlayBackground }"
    @mousedown="onMouseDown"
  >
    <div class="display-content">
      <div class="run-title" v-if="showRunTitle">
        {{ runTitle }}
      </div>

      <div class="splits-table" v-if="showSplits">
        <div
          v-for="split in paginatedSplits"
          :key="split.id"
          class="split-row"
          :class="{
            completed: split.is_completed,
            'with-border': showSplitSeparators
          }"
        >
          <div class="split-name" :title="split.name">{{ split.name }}</div>

          <div
            class="split-time"
            :class="{
              'delta-gold':
                (split.is_completed || !isRunActive) &&
                showComparisonColumns &&
                (isRunActive ? isGold(split.order, split.split_time) : getFrozenGold(split.order)),
              'delta-positive':
                (split.is_completed || !isRunActive) &&
                showComparisonColumns &&
                !(isRunActive ? isGold(split.order, split.split_time) : getFrozenGold(split.order)) &&
                getDisplayDelta(split.order, split.split_time) !== null &&
                getDisplayDelta(split.order, split.split_time)! >= 0,
              'delta-negative':
                (split.is_completed || !isRunActive) &&
                showComparisonColumns &&
                !(isRunActive ? isGold(split.order, split.split_time) : getFrozenGold(split.order)) &&
                getDisplayDelta(split.order, split.split_time) !== null &&
                getDisplayDelta(split.order, split.split_time)! < 0
            }"
          >
            <template v-if="(split.is_completed || !isRunActive) && showComparisonColumns && getDisplayDelta(split.order, split.split_time) !== null">
              {{ formatDelta(getDisplayDelta(split.order, split.split_time)!) }}
            </template>
            <template v-else-if="split.split_time !== null">
              {{ formatTime(split.split_time) }}
            </template>
          </div>

          <div class="split-best" v-if="showComparisonColumns">
            {{ getBestSplitTime(split.order) !== null ? formatTime(getBestSplitTime(split.order)!) : '' }}
          </div>
        </div>
      </div>
      
      <div class="sum-best" v-if="showSumOfBest && sumOfBest !== null && hasBestRun">
        {{ $t('sum_of_best') }}: {{ formatTime(sumOfBest) }}
      </div>
      <div class="predicting-timer" v-if="showPredictingTimer">
        {{ $t('predicting') }}: {{ formatTime(disruptionPredictedTime!) }}
      </div>
      <div class="timer" v-if="showTimer">{{ displayTimer }}</div>
      <div class="pending-groups" v-if="showGroupList">
        <div
          v-for="group in overlayState.pending_groups"
          :key="group.group_id"
          class="pending-group-row"
        >
          <span class="pending-group-name">{{ group.first_split_name }}</span>
        </div>
      </div>
      <div class="failures" v-if="showRunAborted">{{ $t('failures') }}: {{ abortCount }}</div>
      <div class="sum-last" v-if="showSumLast">
        <div class="sum-last-label">{{ $t('sum_of_the_last') }}:</div>
        <div class="sum-last-times">
          <span
            v-for="(run, idx) in lastRuns"
            :key="run.id"
            class="sum-last-item"
          > {{ formatTime(run.total_time) }}
            <span v-if="idx < lastRuns.length - 1">+</span>
          </span>
        </div>
        <div class="sum-last-total">= {{ formatTime(sumLast!) }}</div>
      </div>
    </div>

    <div v-if="settings.overlay.drag_mode" class="drag-overlay">
      <p>{{ $t('toggle_mode') }}: {{ formatShortcutDisplay(settings.overlay.toggle_mode_key) }}</p>
    </div>
  </div>
</template>

<style scoped>
.overlay-window {
  height: max-content;
  width: 300px;
  min-height: 20px;
  position: relative;
  display: inline-block;
  color: white;
}

.overlay-window.drag-mode {
  cursor: move;
}

.display-content {
  min-width: 150px;
  max-width: 300px;
  position: relative;
  z-index: 1;
  pointer-events: none;
  user-select: none;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.run-title {
  text-align: center;
  margin-bottom: 5px;
  color: white;
  text-shadow: 1px 1px 2px rgba(0, 0, 0);
  word-wrap: break-word;
  white-space: normal;
  max-width: 500px;
}

.splits-table {
  width: 100%;
  min-width: 0;
  text-shadow: 1px 1px 2px rgba(0, 0, 0);
}

.split-row {
  display: flex;
  padding: 5px 5px;
  min-width: 0;
  gap: 6px;
}

.split-row.with-border {
  border-bottom: 1px solid rgba(126, 126, 126);
}
.split-row.with-border:first-child {
  border-top: 1px solid rgba(126, 126, 126);
}

.split-row.completed .split-name {
  color: #90ee90;
}

.split-name {
  text-align: left;
  flex: 1 1 auto;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: white;
  font-size: 15px;
}

.split-time {
  text-align: right;
  font-family: monospace;
  flex: 0 0 auto;
  white-space: nowrap;
  font-size: 15px;
}

.split-time.delta-gold {
  color: #ffd700;
}
.split-time.delta-positive {
  color: #e05050;
}
.split-time.delta-negative {
  color: #90ee90;
}

.split-best {
  text-align: right;
  flex: 0 0 auto;
  white-space: nowrap;
  color: white;
  font-family: monospace;
  font-size: 15px;
}

.timer{
  padding: 5px;
  font-size: 30px;
}

.timer,
.failures,
.sum-best,
.predicting-timer {
  align-self: end;
  padding-right: 5px;
  color: white;
  font-family: monospace;
  text-shadow: 1px 1px 2px rgba(0, 0, 0);
}
.sum-best,
.failures,
.predicting-timer {
  font-size: 15px;
}

.sum-last {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-shadow: 1px 1px 2px rgba(0,0,0,1);
  margin-top: 3px;
}
.sum-last-label{
  font-size: 15px;
}
.sum-last-times {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 2px; 
}
.sum-last-times > span,
.sum-last-times > span > span{
  font-size: 15px;  
}
.sum-last-total {
  font-size: 15px; 
  font-weight: bold;
  margin-top: 2px;
}

.pending-groups {
  width: 100%;
  text-shadow: 1px 1px 2px rgba(0, 0, 0);
  color: rgb(0, 204, 255);
}

.pending-group-row {
  display: flex;
  padding: 0px 5px;
}

.pending-group-name {
  text-align: left;
  flex: 1 1 auto;
  white-space: wrap;
  font-size: 15px;
}

.drag-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  border: 2px dashed white;
  z-index: 2;
  pointer-events: auto;
}
.drag-overlay > p {
  text-wrap: nowrap;
  color: yellow;
}
</style>