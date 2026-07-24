<script setup lang="ts">
import { watch, onMounted, ref, computed, onUnmounted} from 'vue'
import { listen } from '@tauri-apps/api/event'
import { settings } from "../services/settings"
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import KeybindCapture from '../views/components/KeybindCapture.vue'
import { invoke } from "@tauri-apps/api/core"
import { updateGlobalShortcut } from "../services/settings"
import type { AppSettings } from "../types/settings"
import { provideKeybindContext } from '../composables/useKeybindContext'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

provideKeybindContext()

type OverlayBrowserStatusPayload =
  | { status: 'Stopped' }
  | { status: 'Starting' }
  | { status: 'Running' }
  | { status: 'Error'; message: string }

const overlayBrowserStatus = ref<OverlayBrowserStatusPayload>({ status: 'Stopped' })
let unlistenStatus: (() => void) | null = null

const addressDisplay = computed({
  get() {
    if (settings.overlay.overlay_browser) {
      switch (overlayBrowserStatus.value.status) {
        case 'Starting': return t('overlay_browser_starting')
        case 'Running': return t('overlay_browser_running')
        case 'Error': return t(`overlay_browser_error`)
      }
    }
    return settings.overlay.overlay_browser_addr
  },
  set(val: string) {
    settings.overlay.overlay_browser_addr = val
  }
})
const addressErrorTooltip = computed(() => {
  return overlayBrowserStatus.value.status === 'Error'
    ? overlayBrowserStatus.value.message
    : undefined
})

const syncOverlayVisibility = async () => {
  const overlayWindow = await WebviewWindow.getByLabel('overlay-window')
  if (!overlayWindow) return

  if (settings.overlay.show) {
    await overlayWindow.show()
  } else {
    await overlayWindow.hide()
  }
}

const resetOverlaySettings = async () => {
  const defaults = await invoke<AppSettings>("get_default_settings")
  Object.assign(settings.overlay, defaults.overlay)
  await invoke("set_settings", { newSettings: { ...settings } })
  await updateGlobalShortcut()
}

const preventInput = (event: KeyboardEvent) => {
  if (event.key === '.' ||
    event.key === ',' ||
    event.key === '+' ||
    event.key === '-')
  {
    event.preventDefault()
  }
}

watch(
  () => settings.overlay.show,
  async () => {
    await syncOverlayVisibility()
  }
)

onMounted(async () => {
  await syncOverlayVisibility()

  try {
    overlayBrowserStatus.value = await invoke<OverlayBrowserStatusPayload>('get_overlay_browser_status')
  } catch {}

  unlistenStatus = await listen<OverlayBrowserStatusPayload>('overlay-browser-status', (event) => {
    overlayBrowserStatus.value = event.payload
  })
})

onUnmounted(() => {
  if (unlistenStatus) unlistenStatus()
})
</script>

<template>
  <div class="overlay">
    <div>
      <div class="enable-overlay">
        <label class="custom-checkbox enable-overlay-checkbox">
          {{ $t('enable_overlay') }}
          <input
            type="checkbox"
            v-model="settings.overlay.show"
          />
          <span class="checkmark"></span>
        </label>
      </div>

      <div class="enable-overlay-desc">
        {{ $t('enable_desc') }}
      </div>
    </div>

    
    <fieldset :disabled="!settings.overlay.show" :style="{ '--cb-cursor': !settings.overlay.show ? 'default' : 'pointer' }">  
      <div class="overlay-browser">
        {{ $t('overlay_browser') }}:
        <input
          type="text"
          placeholder="127.0.0.1:5750"
          class="overlay-browser-addr"
          :disabled="settings.overlay.overlay_browser"
          v-model="addressDisplay"
          :title="addressErrorTooltip"
          />
        <label class="custom-checkbox">
          <input
            type="checkbox"
            v-model="settings.overlay.overlay_browser"
          />
          <span class="checkmark"></span>
        </label>
        
        <div class="overlay-browser-desc">
          {{ $t('overlay_browser_desc') }}
        </div>
      </div>
      
      <div class="overlay-transparent"> {{ $t('overlay_transparent') }}
        <input
          type="range"
          min="0"
          max="100"
          step="1"
          v-model="settings.overlay.overlay_transparent"
          @input="(e) => settings.overlay.overlay_transparent = parseInt((e.target as HTMLInputElement).value)"
        > {{ settings.overlay.overlay_transparent }}%
      </div>

      <div class="run-name">
        <label class="custom-checkbox">
          {{ $t('run_name') }}
          <input
            type="checkbox"
            v-model="settings.overlay.run_name"
          />
          <span class="checkmark"></span>
        </label>
      </div>

      <div class="show-splits">
        <label class="custom-checkbox">
          {{ $t('show_splits') }}
          <input
            type="checkbox"
            v-model="settings.overlay.show_splits"
          />
          <span class="checkmark"></span>
        </label>
      </div>

      <div class="number-of-splits">
        {{ $t('number_of_splits') }}:
        <input
          type="number"
          class="input-number-of-splits"
          v-model="settings.overlay.number_of_splits"
          @keypress="preventInput"
        >
        <div class="number-of-splits-desc">
          {{ $t('number_of_splits_desc') }}
        </div>
      </div>

      <div class="time-accuracy">
        {{ $t('time_accuracy') }}:
        <select
          id="accuracy"
          v-model="settings.overlay.time_accuracy"
        >
          <option value="seconds">
            {{ $t('accuracy_s') }}
          </option>
          <option value="tenths">
            {{ $t('accuracy_t') }}
          </option>
          <option value="hundredths">
            {{ $t('accuracy_h') }}
          </option>
          <option value="milliseconds">
            {{ $t('accuracy_m') }}
          </option>
        </select>
      </div>

      <div class="time-gold">
        {{ $t('time_gold') }}:
        <select
          id="gold"
          v-model="settings.overlay.time_gold"
        >
          <option value="segments">
            {{ $t('gold_segments') }}
          </option>
          <option value="splits">
            {{ $t('gold_splits') }}
          </option>
        </select>
      </div>

      <div class="split-separators">
        <label class="custom-checkbox">
          {{ $t('split_separators') }}
          <input
            type="checkbox"
            v-model="settings.overlay.split_separators"
          />
          <span class="checkmark"></span>
        </label>
      </div>

      <div class="show-groups">
        <label class="custom-checkbox">
          {{ $t('show_groups') }}
          <input
            type="checkbox"
            v-model="settings.overlay.group_list"
          />
          <span class="checkmark"></span>
        </label>
      </div>

      <div class="sum-of-best">
        <label class="custom-checkbox">
          {{ $t('sum_of_best_desc') }} ({{ $t('sum_of_best') }})
          <input
            type="checkbox"
            v-model="settings.overlay.sum_of_best"
          />
          <span class="checkmark"></span>
        </label>
      </div>

      <div class="predicting-timer">
        <label class="custom-checkbox">
          {{ $t('predicting_timer') }}
          <input
            type="checkbox"
            v-model="settings.overlay.predicting_timer"
          />
          <span class="checkmark"></span>
        </label>

        <div class="predicting-timer-desc">
          {{ $t('predicting_timer_desc') }}
        </div>
      </div>

      <div class="fake-timer">
        <label class="custom-checkbox">
          {{ $t('fake_timer') }}
          <input
            type="checkbox"
            v-model="settings.overlay.fake_timer"
          />
          <span class="checkmark"></span>
        </label>

        <div class="fake-timer-desc">
          {{ $t('fake_timer_desc') }}
        </div>
      </div>

      <div class="run-aborted">
        <label class="custom-checkbox">
          {{ $t('run_aborted') }}
          <input
            type="checkbox"
            v-model="settings.overlay.run_aborted"
          />
          <span class="checkmark"></span>
        </label>
      </div>

      <div class="sum-of-last-runs">
        {{ $t('sum_of_the_last_desc') }} ({{ $t('sum_of_the_last') }})
        <input
          type="number"
          class="input-sum-of-last-runs"
          v-model="settings.overlay.sum_of_the_last"
          @keypress="preventInput"
        >
      </div>

      <div class="overlay-separator"></div>

      <p>
        {{ $t('keyboard_shortcuts') }}:
      </p>

      <div class="toggle-key">
        <label>{{ $t('toggle_overlay_view') }}:</label>
        <KeybindCapture 
          v-model="settings.overlay.toggle_visibility_key"
          :disabled="!settings.overlay.show"
          :other-keys="[settings.overlay.toggle_mode_key, settings.overlay.run_reset_key]"
        />
      </div>

      <div class="toggle-key">
        <label>{{ $t('toggle_overlay_mode') }}:</label>
        <KeybindCapture 
          v-model="settings.overlay.toggle_mode_key"
          :disabled="!settings.overlay.show"
          :other-keys="[settings.overlay.toggle_visibility_key, settings.overlay.run_reset_key]"
        />
      </div>

      <div class="toggle-key">
        <label>{{ $t('run_reset') }}:</label>
        <KeybindCapture 
          v-model="settings.overlay.run_reset_key"
          :disabled="!settings.overlay.show"
          :other-keys="[settings.overlay.toggle_visibility_key, settings.overlay.toggle_mode_key]"
        />
      </div>

      <div class="overlay-separator"></div>

      <div class="reset-overlay-settings">
        <button @click="resetOverlaySettings">{{ $t('reset_overlay_settings') }}</button>
      </div>
    </fieldset>
  </div>
</template>

<style scoped>
.overlay,
fieldset{
  display: flex;
  flex-direction: column;
  width: 100%;
}
.overlay > div,
fieldset > div{
  padding: 10px;
  gap: 5px;
}
p{
  margin: 0px;
  padding: 10px;
}
.overlay > div:hover,
fieldset > div:hover{
  background-color: var(--stngs-bg-color);
}
.overlay-transparent > input{
  width: 150px;
}
.toggle-key{
  display: flex;
  flex-direction: row;
  align-items: center;
  width: 100%;
}
.input-number-of-splits,
.input-sum-of-last-runs,
.overlay-browser-addr{
  box-shadow: 0px 1px 0px gray;
}
.overlay-browser-addr{
  background-color: var(--bg-color);
  width: 150px;
  margin-right: 10px;
}
.enable-overlay-desc,
.overlay-browser-desc,
.number-of-splits-desc,
.fake-timer-desc,
.predicting-timer-desc{
  font-size: 15px;
  margin-top: 5px;
  white-space: pre-line;
}

.reset-overlay-settings button{
  height: 30px;
}

.overlay-separator{
  background-color: rgba(0, 0, 0, 0.5);
  padding: 0px;
  margin: 10px 0 0 3px;
  height: 3px;
}
.overlay-separator:hover{
  background-color: rgba(0, 0, 0, 0.5) !important;
}
.custom-checkbox,
.custom-checkbox input,
.overlay-transparent input {
  cursor: var(--cb-cursor);
}
</style>