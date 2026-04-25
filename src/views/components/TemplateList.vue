<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import { useTooltip } from '../../composables/useTooltip'

import imgConflict from '../../imgs/conflict.png'


const { t } = useI18n()
const STORAGE_KEY_TEMPLATE_SEARCH = 'templates_search_text'

const searchInputRef = ref<HTMLInputElement | null>(null)
const defaultSearchInputRef = ref<HTMLInputElement | null>(null)

interface Template {
  id: string
  name: string
  is_active: boolean
  non_sequential_mode: boolean
  exclude_time_between_groups: boolean
  groups: any[]
}

const searchTemplateText = ref(localStorage.getItem(STORAGE_KEY_TEMPLATE_SEARCH) || '')
const templates = ref<Template[]>([])

const loadTemplates = async () => {
  templates.value = await invoke('get_templates')
}

onMounted(loadTemplates)

const filteredTemplates = computed(() =>
  templates.value.filter(t =>
    t.name.toLowerCase().includes(searchTemplateText.value.toLowerCase())
  )
)

const toggleActive = async (template: Template) => {
  if (isToggleDisabled(template)) return
  const newState = !template.is_active
  template.is_active = newState
  await invoke('update_template', { template })
}

const emit = defineEmits(['edit', 'create'])

const getMissionCodes = (template: Template): string[] => {
  const codes: string[] = []

  for (const g of template.groups) {
    const start = g.steps?.find((s: any) => s.type === 'StartMission')
    if (start?.mission_code) {
      const parts = start.mission_code.split('&&').map((p: string) => p.trim()).filter(Boolean)
      codes.push(...parts)
    }
  }

  return codes
}

const missionCodeIndex = computed(() => {
  const map = new Map<string, Template[]>()
  for (const template of templates.value) {
    const codes = getMissionCodes(template)
    for (const code of codes) {
      if (!map.has(code)) {
        map.set(code, [])
      }
      map.get(code)!.push(template)
    }
  }
  return map
})

const conflictMap = computed(() => {
  const result: Record<string, Template[]> = {}
  for (const template of templates.value) {
    const codes = getMissionCodes(template)
    const conflictsSet = new Set<string>()
    for (const code of codes) {
      const list = missionCodeIndex.value.get(code)
      if (!list) continue
      for (const t of list) {
        if (t.id === template.id) continue
        if (!t.is_active) continue
        conflictsSet.add(t.id)
      }
    }
    result[template.id] = Array.from(conflictsSet).map(id => 
      templates.value.find(t => t.id === id)!
    )
  }
  return result
})

const isToggleDisabled = (template: Template) => {
  if (template.is_active) return false
  return conflictMap.value[template.id]?.length > 0
}

const getConflictTooltip = (template: Template) => {
  const conflicts = conflictMap.value[template.id]
  if (!conflicts || conflicts.length === 0) return ''
  const names = conflicts.map(t => `• ${t.name}`).join('\n')
  return `${t('conflict_with')}:\n${names}\n\n${t('deactivate_conflicting_templates')}`
}

const deactivateConflicts = async (template: Template) => {
  const conflicts = conflictMap.value[template.id]
  if (!conflicts || conflicts.length === 0) return
  for (const t of conflicts) {
    t.is_active = false
    await invoke('update_template', { template: t })
  }
}
const handleConflictClick = async (template: Template) => {
  hideTooltip()
  await deactivateConflicts(template)
}

const defaultTemplates = ref<Template[]>([])
const defaultSearch = ref('')
const dropdownOpen = ref(false)
const addWrapperRef = ref<HTMLElement | null>(null)
const addingTemplates = ref<Set<string>>(new Set())

const loadDefaultTemplates = async () => {
  defaultTemplates.value = await invoke('get_default_templates')
}

onMounted(async () => {
  await loadTemplates()
  await loadDefaultTemplates()
})

const getFirstMissionCode = (template: Template) => {
  const g = template.groups?.[0]
  if (!g) return null
  const step = g.steps?.find((s:any) => s.type === 'StartMission')
  return step?.mission_code ?? null
}

const availableDefaultTemplates = computed(() => {
  return defaultTemplates.value.filter(dt => {
    const dtCode = getFirstMissionCode(dt)
    const exists = templates.value.some(t => {
      const tCode = getFirstMissionCode(t)
      return (
        t.name === dt.name &&
        tCode === dtCode
      )
    })
    return !exists
  })
})
const filteredDefaultTemplates = computed(() =>
  availableDefaultTemplates.value.filter(t =>
    t.name.toLowerCase().includes(
      defaultSearch.value.toLowerCase()
    )
  )
)
const addDefaultTemplate = async (template: Template) => {
  if (addingTemplates.value.has(template.name)) return

  addingTemplates.value.add(template.name)

  try {
    await invoke('import_default_template', { template })
    await loadTemplates()
  } finally {
    addingTemplates.value.delete(template.name)
  }
}
const hasAvailableDefaults = computed(() =>
  availableDefaultTemplates.value.length > 0
)
const handleClickOutside = (event: MouseEvent) => {
  if (!dropdownOpen.value) return

  const el = addWrapperRef.value
  if (!el) return

  if (!el.contains(event.target as Node)) {
    dropdownOpen.value = false
  }
}
onMounted(() => {
  document.addEventListener('click', handleClickOutside)
})

onBeforeUnmount(() => {
  document.removeEventListener('click', handleClickOutside)
})

watch(searchTemplateText, (newVal) => {
  localStorage.setItem(STORAGE_KEY_TEMPLATE_SEARCH, newVal)
})

const {
  tooltipEl,
  tooltipState,
  showTooltip,
  moveTooltip,
  hideTooltip
} = useTooltip()
</script>

<template>
  <div class="mission-templates">
    <div class="template-menu">
      <input
        ref="searchInputRef"
        type="text"
        class="template-search"
        :placeholder="$t('search')"
        v-model="searchTemplateText"
      >
      <button class="template-search-clear" @click="searchTemplateText = ''; searchInputRef?.focus()">⌫</button>
      <button class="add-template" @click="emit('create')">
        {{ $t('create_template') }}
      </button>
      <div class="add-wrapper" ref="addWrapperRef">
        <button
          class="add-default-templates"
          :disabled="!hasAvailableDefaults"
          @click="dropdownOpen = !dropdownOpen"
        >
          {{ $t('add_default') }} {{ dropdownOpen ? '▲' : '▼' }}
        </button>

        <div v-if="dropdownOpen && hasAvailableDefaults" class="default-dropdown">
          <div class="search-default-template">
            <input
              ref="defaultSearchInputRef"
              v-model="defaultSearch"
              class="dropdown-search"
              type="text"
              :placeholder="$t('search')"
            >
            <button class="default-template-search-clear" @click="defaultSearch = ''; defaultSearchInputRef?.focus()">⌫</button>
          </div>

          <div class="default-template-list">
            <div
              v-for="template in filteredDefaultTemplates"
              :key="template.name"
              class="dropdown-item"
            >
              <span>{{ template.name }}</span>
              <button
                class="add-button"
                :disabled="addingTemplates.has(template.name)"
                @click="addDefaultTemplate(template)"
              >
                +
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="template-list">
      <div v-if="filteredTemplates.length === 0" class="empty-list-message">
        {{ $t('no_templates_found') }}
      </div>
      <div
        v-else
        class="template"
        v-for="template in filteredTemplates"
        :key="template.id"
      >
        <div class="template-name">
          {{ template.name }}
        </div>

        <p class="vertsep"></p>

        <button
          class="edit-template"
          @click="emit('edit', template)"
        >
          {{ $t('edit') }}
        </button>

        <p class="vertsep"></p>
        
        <button
          class="toggle-button"
          :class="{ 'bGreen': template.is_active, 'bRed': !template.is_active }"
          :disabled="isToggleDisabled(template)"
          @click="toggleActive(template)"
          >
          {{ template.is_active ? $t('active') : $t('inactive') }}
        </button>
        <div v-if="isToggleDisabled(template)">
          <p class="vertsep"></p>
          <button
            class="conflict"
            @click.stop="handleConflictClick(template)"
            @mouseenter="showTooltip($event, getConflictTooltip(template))"
            @mousemove="moveTooltip"
            @mouseleave="hideTooltip"
          >
            <img
              class="conflict-icon"
              :src="imgConflict"
            >
          </button>
        </div>
      </div>
    </div>
  </div>
  <div
    v-if="tooltipState.visible"
    ref="tooltipEl"
    class="tooltip"
    :style="{
      left: tooltipState.x + 'px',
      top: tooltipState.y + 'px'
    }"
  >
    {{ tooltipState.text }}
  </div>
</template>

<style scoped>
.mission-templates{
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
}

.template-menu{
  display: flex;
  flex-direction: row;
  flex-wrap: wrap;
  width: 100%;
  padding: 5px 0px 5px 10px;
  z-index: 10;
  gap: 10px;
  box-shadow: 3px 3px 0 0 rgba(0,0,0,0.5); 
}
.template-menu > *{
  min-height: 30px;
}
.template-search-clear{
  margin-left: -10px;
}
.template-search{
  text-align: center;
}

.empty-list-message{
  white-space: pre;
  line-height: 1.5;
  text-wrap: wrap;
  width: 95%;
}
.template-list{
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  height: 100%;
  width: 100%;
  gap: 10px;
  padding: 10px;
}
.template > div:first-child{
  padding-left: 10px;
}
.template{
  display: flex;
  flex-direction: row;
  width: fit-content;
  height: 30px;
  align-items: center;
  background-color: var(--card-bg);
  box-shadow: 0 0 0 2px rgba(0,0,0,0.5);
}
.template > div, .template > button{
  text-wrap: nowrap;
  padding: 0 10px;
}
.template > div:last-child{
  display: flex;
  height: 100%;
  padding: 0px;
}
.edit-template,
.toggle-button{
  height: 30px;
}

.toggle-button{
  transition: background-color 0.3s;
  padding: 0 10px;
}
.toggle-button:disabled{
  opacity:0.4;
}
.toggle-button:disabled:hover{
  background-color: var(--btn-bg-color);
}

.conflict{
  display: flex;
  height: 100%;
  width: 35px;
  position: relative;
  align-items: center;
  justify-content: center;
}
.conflict-icon{
  height:25px;
  width:25x;
}
.tooltip{
  position: fixed;
  z-index: 9999;
  min-width: 10px;
  max-width: 1000px;
  white-space: pre;
  padding: 5px 10px;
  background: var(--card-bg);
  color: var(--text-color);
  box-shadow: 0 0 4px 1px rgba(0,0,0,1);
  font-size: 13px;
}

.add-wrapper{
  position: relative;
}
.add-default-templates{
  height: 100%;
}
.add-default-templates:disabled{
  opacity:0.4;
  cursor:default !important;
}
.default-dropdown{
  position:absolute;
  top:100%;
  right:0;
  display:flex;
  flex-direction:column;
  gap:6px;
  margin-top:10px;
  background: var(--bg-color);
  width: 280px;
  box-shadow: 0 0 3px 1px rgba(0,0,0,1);
}
.default-dropdown{
  animation:dropdown-appear 0.2s ease-out;
}

@keyframes dropdown-appear{
  from{
    opacity:0;
    transform:translateY(-10px);
  }
  to{
    opacity:1;
    transform:translateY(0);
  }
}
.search-default-template{
  display: flex;
  flex-direction: row;
  width: fit-content;
  align-self:flex-end;
  height: 30px;
}
.dropdown-search{
  text-align: center;
}
.default-template-search-clear{
  padding: 0px;
  width: 30px;
}
.default-template-list{
  display:flex;
  flex-direction:column;
  gap:6px;
  max-height:70vh;
  overflow-y:auto;
  padding: 2px;
}
.dropdown-item{
  display:flex;
  flex-direction: row;
  padding-left: 5px;
  background:var(--card-bg);
  box-shadow: 0 0 3px 1px rgba(0,0,0,1);
}
.dropdown-item span {
  flex: 1;
  padding: 4px 0;
}
.dropdown-item:hover{
  background-color: var(--btn-bg-color);
}
.add-button{
  width: 30px;
  min-height: 30px;
  margin-left: 10px;
  font-size: 30px;
}
</style>