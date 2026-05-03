<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import {
  useKeybindActiveId,
  MODIFIER_CODES, IGNORED_CODES,
  eventToShortcutString, modifierOnlyShortcutFromKeyup, modifierParts,
  formatShortcutDisplay,
} from '../../composables/useKeybindContext'

const props = defineProps<{
  modelValue: string
  disabled?: boolean
  otherKeys?: string[]
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

const MY_ID = Symbol()
const activeId = useKeybindActiveId()

const isCapturing = computed({
  get: () => activeId.value === MY_ID,
  set: (val: boolean) => { activeId.value = val ? MY_ID : null },
})

const currentKey    = ref(props.modelValue)
const activeMods    = ref<string[]>([])
const conflictError = ref(false)
let regularKeyPressed = false

watch(() => props.modelValue, (v) => {
  if (!isCapturing.value) { currentKey.value = v; conflictError.value = false }
})

function tryCommit(pluginStr: string, displayStr: string) {
  if (props.otherKeys?.includes(pluginStr)) {
    conflictError.value = true
    stopCapture()
    return
  }
  conflictError.value = false
  currentKey.value = displayStr
  emit('update:modelValue', pluginStr)
  stopCapture()
}

const handleKeyDown = (e: KeyboardEvent) => {
  if (!isCapturing.value) return
  e.preventDefault(); e.stopPropagation()

  if (IGNORED_CODES.has(e.code)) return

  if (e.code === 'Escape' && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
    stopCapture(); return
  }

  activeMods.value = modifierParts(e)

  if (MODIFIER_CODES.has(e.code)) return

  regularKeyPressed = true
  const pluginStr = eventToShortcutString(e)
  if (!pluginStr) return

  const displayStr = formatShortcutDisplay(pluginStr)
  tryCommit(pluginStr, displayStr)
}

const handleKeyUp = (e: KeyboardEvent) => {
  if (!isCapturing.value) return
  e.preventDefault(); e.stopPropagation()

  if (IGNORED_CODES.has(e.code) || !MODIFIER_CODES.has(e.code)) return

  activeMods.value = modifierParts(e)

  if (regularKeyPressed) return

  if (e.ctrlKey || e.altKey || e.shiftKey || e.metaKey) return

  const pluginStr = modifierOnlyShortcutFromKeyup(e)
  if (!pluginStr) return
  tryCommit(pluginStr, pluginStr)
}

const stopCapture = () => {
  isCapturing.value = false
  activeMods.value = []
  regularKeyPressed = false
  window.removeEventListener('keydown', handleKeyDown, true)
  window.removeEventListener('keyup',   handleKeyUp,   true)
}

const startCapture = () => {
  if (props.disabled || isCapturing.value) return
  conflictError.value = false
  regularKeyPressed = false
  isCapturing.value = true
  window.addEventListener('keydown', handleKeyDown, true)
  window.addEventListener('keyup',   handleKeyUp,   true)
}

const capturePreview = computed(() =>
  activeMods.value.length ? activeMods.value.join('+') + '+...' : null
)
const displayValue = computed(() => formatShortcutDisplay(currentKey.value))
</script>

<template>
  <div
    class="keybind-capture"
    :class="{ capturing: isCapturing, disabled, conflict: conflictError }"
    @click="startCapture"
    :title="conflictError ? $t('shortcut_conflict') : undefined"
  >
    <span v-if="!isCapturing" class="key-value">{{ displayValue }}</span>
    <span v-else class="key-placeholder">{{ capturePreview ?? $t('press_key') }}</span>
  </div>
</template>

<style scoped>
.keybind-capture {
  cursor: pointer;
  min-width: 80px;
  height: 28px;
  padding: 0 5px;
  text-align: center;
  align-items: center;
  align-self: center;
  align-content: center;
  background-color: var(--btn-bg-color);
  transition: 0.2s;
}
.keybind-capture:hover:not(.disabled) {
  background-color: var(--btn-bg-hover-color);
  transition: 0.2s;
}
.keybind-capture.capturing {
  animation: pulse 1s infinite;
}
.keybind-capture.disabled {
  cursor: default;
}
.keybind-capture.conflict {
  outline: 2px solid #e05050;
}
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50%       { opacity: 0.5; }
}
</style>