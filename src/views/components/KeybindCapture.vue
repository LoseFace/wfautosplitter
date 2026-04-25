<script setup lang="ts">
import { ref } from 'vue'

const props = defineProps<{
  modelValue: string
  disabled?: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

const isCapturing = ref(false)
const currentKey = ref(props.modelValue)


const formatKey = (event: KeyboardEvent): string => {
  return event.key.length === 1
    ? event.key.toUpperCase()
    : event.key
}

const handleKeyDown = (event: KeyboardEvent) => {
  if (!isCapturing.value) return
  
  event.preventDefault()
  event.stopPropagation()
  
  if (event.key === 'Delete') {
    isCapturing.value = false
    window.removeEventListener('keydown', handleKeyDown)
    return
  }
  
  const newKey = formatKey(event)
  currentKey.value = newKey
  emit('update:modelValue', newKey)
  
  isCapturing.value = false
  window.removeEventListener('keydown', handleKeyDown)
}

const startCapture = () => {
  if (props.disabled) return
  if (isCapturing.value) return
  
  isCapturing.value = true
  window.addEventListener('keydown', handleKeyDown)
}
</script>

<template>
  <div 
    class="keybind-capture"
    :class="{ 
      'capturing': isCapturing,
      'disabled': disabled 
    }"
    @click="startCapture"
  >
    <span v-if="!isCapturing" class="key-value">{{ currentKey }}</span>
    <span v-else class="key-placeholder">{{ $t('press_key') }}</span>
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

@keyframes pulse {
  0%, 100% {
    opacity: 1;
  }
  50% {
    opacity: 0.5;
  }
}
</style>