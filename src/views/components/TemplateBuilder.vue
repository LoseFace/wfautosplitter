<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { useTooltip } from '../../composables/useTooltip'

import imgGarbage from '../../imgs/garbage.png'
import imgPlus from '../../imgs/plus.png'
 
const { t } = useI18n()
 
const templateNameInput = ref<HTMLInputElement | null>(null)
const startNameInputs = ref<Map<string, HTMLInputElement>>(new Map())
const startCodeInputs = ref<Map<string, HTMLInputElement>>(new Map())
const missionCodeInputs = ref<Map<string, HTMLInputElement>>(new Map())
const splitNameInputs = ref<Map<string, HTMLInputElement>>(new Map())
const splitCodeInputs = ref<Map<string, HTMLInputElement>>(new Map())
const finishNameInputs = ref<Map<string, HTMLInputElement>>(new Map())
const finishCodeInputs = ref<Map<string, HTMLInputElement>>(new Map())
const groupEls = ref<Map<string, HTMLElement>>(new Map())
const splitEls = ref<Map<string, HTMLElement>>(new Map())
 
interface Split {
  name: string
  code: string
}
 
interface Group {
  id: string
  startName: string
  startCode: string
  missionCode: string
  finishName: string
  finishCode: string
  splits: Split[]
}
 
const templateName = ref('')
 
const groups = ref<Group[]>([
  {
    id: crypto.randomUUID(),
    startName: '',
    startCode: '',
    missionCode: '',
    finishName: '',
    finishCode: '',
    splits: []
  }
])
 
const addGroup = () => {
  groups.value.push({
    id: crypto.randomUUID(),
    startName: '',
    startCode: '',
    missionCode: '',
    finishName: '',
    finishCode: '',
    splits: []
  })
}
 
const deleteGroup = (groupId: string) => {
  groups.value = groups.value.filter(g => g.id !== groupId)
}
 
const addSplit = (groupId: string) => {
  const group = groups.value.find(g => g.id === groupId)
  if (group) {
    group.splits.push({
      name: '',
      code: ''
    })
  }
}
 
const deleteSplit = (groupId: string, index: number) => {
  const group = groups.value.find(g => g.id === groupId)
  if (group) {
    group.splits.splice(index, 1)
  }
}
 
const props = defineProps<{
  templateData: any | null
}>()
 
const emit = defineEmits(['back'])
 
const sequentialMode = ref(false)
const excludeTimeBetweenGroups = ref(false)
 
const editingTemplateId = ref<string | null>(null)
 
const isEditing = computed(() => editingTemplateId.value !== null)
 
const buildTemplatePayload = () => {
  const groupsPayload = groups.value.map(g => {
    let order = 0
    const steps = []
 
    steps.push({
      type: 'StartMission',
      order: order++,
      trigger_keyword: g.startCode,
      split_name: g.startName,
      mission_code: g.missionCode || null
    })
 
    for (const s of g.splits) {
      steps.push({
        type: 'MissionSplit',
        order: order++,
        split_name: s.name,
        trigger_keyword: s.code
      })
    }
 
    steps.push({
      type: 'EndMission',
      order: order++,
      split_name: g.finishName,
      trigger_keyword: g.finishCode
    })
 
    return {
      id: crypto.randomUUID(),
      steps
    }
 
  })
 
  return {
    id: editingTemplateId.value ?? '',
    name: templateName.value,
    is_active: false,
    sequential_mode: sequentialMode.value,
    exclude_time_between_groups: excludeTimeBetweenGroups.value,
    groups: groupsPayload
  }
}
 
const saveTemplate = async () => {
  if (!templateName.value) {
    alert('Template name required')
    return
  }

  const payload = buildTemplatePayload()

  try {
    if (isEditing.value) {
      if (templateName.value !== props.templateData?.name) {
        await invoke('delete_template', { templateId: editingTemplateId.value })
        payload.id = crypto.randomUUID()
        await invoke('create_template', { template: payload })
      } else {
        await invoke('update_template', { template: payload })
      }
    } else {
      await invoke('create_template', { template: payload })
    }
    emit('back')
  } catch (e) {
    console.error(e)
    alert(String(e))
  }
}
 
const deleteTemplateFromDb = async () => {
  if (!editingTemplateId.value) return
 
  try {
    await invoke('delete_template', {
      templateId: editingTemplateId.value
    })
    emit('back')
 
  } catch (e) {
    console.error(e)
  }
}
 
watch(() => props.templateData, (template) => {
  if (!template) return
 
  editingTemplateId.value = template.id
  templateName.value = template.name
  sequentialMode.value = template.sequential_mode
  excludeTimeBetweenGroups.value = template.exclude_time_between_groups
 
  groups.value = template.groups.map((g: any) => {
    const start = g.steps.find((s: any) => s.type === 'StartMission')
    const finish = g.steps.find((s: any) => s.type === 'EndMission')
    const splits = g.steps.filter((s: any) => s.type === 'MissionSplit')
 
    return {
      id: crypto.randomUUID(),
      startName: start?.split_name ?? '',
      startCode: start?.trigger_keyword ?? '',
      missionCode: start?.mission_code ?? '',
      finishName: finish?.split_name ?? '',
      finishCode: finish?.trigger_keyword ?? '',
      splits: splits.map((s: any) => ({
        name: s.split_name,
        code: s.trigger_keyword
      }))
    }
  })
}, { immediate: true })
 
const clearField = (obj: any, field: string, inputRef: HTMLInputElement | null) => {
  obj[field] = ''
  inputRef?.focus()
}
 
type DragKind =
  | { type: 'split'; groupId: string; fromIndex: number }
  | { type: 'group'; fromIndex: number }
 
const dragging = ref<DragKind | null>(null)
const splitInsertIndex = ref<number | null>(null)
const groupHoverIndex = ref<number | null>(null)

let ghostEl: HTMLElement | null = null
let offsetX = 0
let offsetY = 0
 
const createGhost = (source: HTMLElement, clientX: number, clientY: number) => {
  const rect = source.getBoundingClientRect()
  ghostEl = source.cloneNode(true) as HTMLElement
  const computedStyle = window.getComputedStyle(source)
  ghostEl.style.cssText = `
    position: fixed;
    pointer-events: none;
    z-index: 99999;
    opacity: 0.7;
    box-shadow: 0 4px 20px rgba(0,0,0,0.6);
    left: ${rect.left}px;
    top: ${rect.top}px;
    background: var(--stngs-bg-color, #2a2a2a);
    outline: 1px solid rgba(0,220,100,0.6);
    display: ${computedStyle.display};
    width: ${computedStyle.width};
    height: ${computedStyle.height};
    gap: ${computedStyle.gap};
    margin: 0;
  `
  document.body.appendChild(ghostEl)
  offsetX = clientX - rect.left
  offsetY = clientY - rect.top
}
 
const moveGhost = (clientX: number, clientY: number) => {
  if (!ghostEl) return
  ghostEl.style.left = `${clientX - offsetX}px`
  ghostEl.style.top  = `${clientY - offsetY}px`
}
 
const removeGhost = () => {
  ghostEl?.remove()
  ghostEl = null
}
 
const onSplitMouseDown = (e: MouseEvent, sourceEl: HTMLElement, groupId: string, fromIndex: number) => {
  if (e.button !== 0) return
  e.preventDefault()
 
  dragging.value = { type: 'split', groupId, fromIndex }
  splitInsertIndex.value = null
  createGhost(sourceEl, e.clientX, e.clientY)
 
  const onMove = (ev: MouseEvent) => {
    moveGhost(ev.clientX, ev.clientY)
 
    const els = Array.from(document.querySelectorAll<HTMLElement>(`[data-split-group="${groupId}"]`))
    let found: number | null = null
    for (const el of els) {
      const r = el.getBoundingClientRect()
      if (ev.clientX >= r.left && ev.clientX <= r.right &&
          ev.clientY >= r.top  && ev.clientY <= r.bottom) {
        found = parseInt(el.dataset.splitIndex!)
        break
      }
    }
    splitInsertIndex.value = (found !== null && found !== fromIndex) ? found : null
  }
 
  const onUp = () => {
    document.removeEventListener('mousemove', onMove)
    document.removeEventListener('mouseup', onUp)
    removeGhost()
 
    const toIndex = splitInsertIndex.value
    if (toIndex !== null && toIndex !== fromIndex) {
      const group = groups.value.find(g => g.id === groupId)
      if (group) {
        const moved = group.splits.splice(fromIndex, 1)[0]
        group.splits.splice(toIndex, 0, moved)
      }
    }
    dragging.value = null
    splitInsertIndex.value = null
  }
 
  document.addEventListener('mousemove', onMove)
  document.addEventListener('mouseup', onUp)
}
 
const draggingSplitGroupId = computed(() =>
  dragging.value?.type === 'split' ? dragging.value.groupId : null
)
const draggingSplitFrom = computed(() =>
  dragging.value?.type === 'split' ? dragging.value.fromIndex : null
)
const draggingGroupFrom = computed(() =>
  dragging.value?.type === 'group' ? dragging.value.fromIndex : null
)
 
const onGroupMouseDown = (e: MouseEvent, sourceEl: HTMLElement, fromIndex: number) => {
  if (e.button !== 0) return
  e.preventDefault()
 
  dragging.value = { type: 'group', fromIndex }
  groupHoverIndex.value = null
  createGhost(sourceEl, e.clientX, e.clientY)
 
  const onMove = (ev: MouseEvent) => {
    moveGhost(ev.clientX, ev.clientY)
 
    const els = Array.from(document.querySelectorAll<HTMLElement>('[data-group-index]'))
    let found: number | null = null
    for (const el of els) {
      const r = el.getBoundingClientRect()
      if (ev.clientX >= r.left && ev.clientX <= r.right &&
          ev.clientY >= r.top  && ev.clientY <= r.bottom) {
        found = parseInt(el.dataset.groupIndex!)
        break
      }
    }
    groupHoverIndex.value = (found !== null && found !== fromIndex) ? found : null
  }
 
  const onUp = () => {
    document.removeEventListener('mousemove', onMove)
    document.removeEventListener('mouseup', onUp)
    removeGhost()
 
    const toIndex = groupHoverIndex.value
    if (toIndex !== null && toIndex !== fromIndex) {
      const moved = groups.value.splice(fromIndex, 1)[0]
      groups.value.splice(toIndex, 0, moved)
    }
    dragging.value = null
    groupHoverIndex.value = null
  }
 
  document.addEventListener('mousemove', onMove)
  document.addEventListener('mouseup', onUp)
}
 
const {
  tooltipEl,
  tooltipState,
  showTooltip,
  moveTooltip,
  hideTooltip
} = useTooltip()
 
const existingTemplates = ref<{ id: string, name: string }[]>([])
const loadTemplates = async () => {
  existingTemplates.value = await invoke('get_templates')
}
 
onMounted(loadTemplates)
const canAddGroup = computed(() => {
  return groups.value.length > 0 && groups.value[0].missionCode.trim() !== ''
})

const missionCodeErrors = computed(() => {
  const errors: string[] = []
  const codes = groups.value.map(g => g.missionCode.trim())
  const anyFilled = codes.some(c => c !== '')

  if (!anyFilled && groups.value.length >= 2) {
    errors.push('• ' + t('enter_mission_code_all_groups'))
    return errors
  }
 
  if (!anyFilled) return errors

  groups.value.forEach((g, index) => {
    if (!g.missionCode.trim()) {
      errors.push('• ' + t('group') + ` ${index + 1}: ` + t('enter_mission_code'))
    }
  })

  const filled = codes.filter(c => c !== '')
  const duplicates = filled.filter((c, i) => filled.indexOf(c) !== i)
  if (duplicates.length > 0) {
    const unique = [...new Set(duplicates)]
    unique.forEach(dup => {
      const conflictGroups = groups.value
        .map((g, i) => g.missionCode.trim() === dup ? i + 1 : null)
        .filter(Boolean)
        .join(', ')
      errors.push('• ' + t('mission_code_conflict') + ` "${dup}" (` + t('group') + `: ${conflictGroups})`)
    })
  }
 
  return errors
})
 
const isNameDuplicate = computed(() => {
  const name = templateName.value.trim().toLowerCase()
 
  return existingTemplates.value.some(t => {
    if (isEditing.value && t.id === editingTemplateId.value) return false
    return t.name.trim().toLowerCase() === name
  })
})
const validationErrors = computed(() => {
  const errors: string[] = []
 
  if (isNameDuplicate.value) {
    errors.push('• ' + t('template_already_exists'))
  }
 
  if (!templateName.value.trim()) {
    errors.push('• ' + t('enter_template_name'))
  }
 
  groups.value.forEach((g, index) => {
    const groupNumber = index + 1
 
    if (!g.startName.trim()) {
      errors.push(`• ` + t('group') + ` ${groupNumber}: ` + t('enter_start_name'))
    }
 
    if (!g.startCode.trim()) {
      errors.push(`• ` + t('group') + ` ${groupNumber}: ` + t('enter_start_code'))
    }
 
    if (!g.finishName.trim()) {
      errors.push(`• ` + t('group') + ` ${groupNumber}: ` + t('enter_finish_name'))
    }
 
    if (!g.finishCode.trim()) {
      errors.push(`• ` + t('group') + ` ${groupNumber}: ` + t('enter_finish_code'))
    }
 
    g.splits.forEach((s, splitIndex) => {
      const splitNumber = splitIndex + 1
      if (!s.name.trim()) {
        errors.push(`• ` + t('group') + ` ${groupNumber}, ` + t('split') + ` ${splitNumber}: ` + t('enter_split_name'))
      }
      if (!s.code.trim()) {
        errors.push(`• ` + t('group') + ` ${groupNumber}, ` + t('split') + ` ${splitNumber}: ` + t('enter_split_code'))
      }
    })
  })
 
  missionCodeErrors.value.forEach(e => errors.push(e))
 
  return errors
})
const validationTooltip = computed(() => {
  if (validationErrors.value.length === 0) return ''
  return validationErrors.value.join('\n')
})
const isTemplateValid = computed(() => {
  return validationErrors.value.length === 0
})
</script>
 
<template>
  <div class="template-builder">
    <div class="template-settings">
      <input
        ref="templateNameInput"
        class="template-name"
        :class="{ 'invalid-input': !templateName.trim() || isNameDuplicate }"
        type="text"
        :placeholder="$t('placeholder_template_name')"
        v-model="templateName">
      <button class="template-name-clear" @click="templateName = ''; templateNameInput?.focus()">⌫</button>
      <label
        class="custom-checkbox sequential-mode"
        @mouseenter="showTooltip($event, $t('sequential_mode_tooltip'))"
        @mousemove="moveTooltip"
        @mouseleave="hideTooltip"
      >
        <input type="checkbox" v-model="sequentialMode"/>{{ $t('sequential_mode') }}
        <span class="checkmark"></span>
      </label>
      <label
        class="custom-checkbox exclude-time-between-groups"
        @mouseenter="showTooltip($event, $t('exclude_time_between_groups_tooltip'))"
        @mousemove="moveTooltip"
        @mouseleave="hideTooltip"
      >
        <input type="checkbox" v-model="excludeTimeBetweenGroups"/>{{ $t('exclude_time_between_groups') }}
        <span class="checkmark"></span>
      </label>
      <div
        class="validation-wrapper"
        @mouseenter="!isTemplateValid && showTooltip($event, validationTooltip)"
        @mousemove="moveTooltip"
        @mouseleave="hideTooltip"
      >
        <button
          class="button button--save"
          :class="{ 'disabled-button': !isTemplateValid }"
          :disabled="!isTemplateValid"
          @click="isTemplateValid && saveTemplate()"
        >
          {{ $t('save') }}
        </button>
      </div>
      <button
        v-if="isEditing"
        class="button button--danger"
        @click="deleteTemplateFromDb()"
      >
        {{ $t('delete') }}
      </button>
      <button
        class="button button--danger"
        @click="emit('back')"
      >
        {{ $t('cancel') }}
      </button>
    </div>
    <div class="template-build">
      <div
        v-for="(group, groupIndex) in groups"
        :key="group.id"
        :ref="(el) => { if (el) groupEls.set(group.id, el as HTMLElement) }"
        class="template-group"
        :data-group-index="groupIndex"
        :class="{
          'drag-source':      draggingGroupFrom === groupIndex,
          'drag-swap-target': groupHoverIndex === groupIndex
        }"
      >
        <div
          class="group-drag-handle"
          @mousedown="onGroupMouseDown($event, groupEls.get(group.id)!, groupIndex)"
        >⠿</div>
        <div class="template-splits">
          <div class="template-start">
            <input
              :ref="(el) => { if (el) startNameInputs.set(group.id, el as HTMLInputElement) }"
              type="text"
              :class="{ 'invalid-input': !group.startName }"
              :placeholder="$t('placeholder_start_name')"
              v-model="group.startName">
            <button class="template-name-clear" @click="clearField(group, 'startName', startNameInputs.get(group.id) ?? null)">⌫</button>
            <input
              :ref="(el) => { if (el) startCodeInputs.set(group.id, el as HTMLInputElement) }"
              type="text"
              :class="{ 'invalid-input': !group.startCode }"
              :placeholder="$t('placeholder_start_code')"
              v-model="group.startCode">
            <button class="template-name-clear" @click="clearField(group, 'startCode', startCodeInputs.get(group.id) ?? null)">⌫</button>
            <input
              :ref="(el) => { if (el) missionCodeInputs.set(group.id, el as HTMLInputElement) }"
              type="text"
              :class="{ 'invalid-input': missionCodeErrors.length > 0 && !group.missionCode.trim() }"
              :placeholder="$t('placeholder_start_mission_code_opt')"
              v-model="group.missionCode">
            <button class="template-name-clear" @click="clearField(group, 'missionCode', missionCodeInputs.get(group.id) ?? null)">⌫</button>
          </div>
  
          <div
            v-for="(split, splitIndex) in group.splits"
            :key="splitIndex"
            :ref="(el) => { if (el) splitEls.set(`${group.id}-${splitIndex}`, el as HTMLElement) }"
            class="template-split"
            :data-split-group="group.id"
            :data-split-index="splitIndex"
            :class="{
              'drag-source':      draggingSplitGroupId === group.id && draggingSplitFrom === splitIndex,
              'drag-swap-target': draggingSplitGroupId === group.id && splitInsertIndex === splitIndex
            }"
          >
            <span
              class="split-drag-handle"
              @mousedown="onSplitMouseDown($event, splitEls.get(`${group.id}-${splitIndex}`)!, group.id, splitIndex)"
            >⠿</span>
            <input
              :ref="(el) => { if (el) splitNameInputs.set(`${group.id}-${splitIndex}`, el as HTMLInputElement) }"
              type="text"
              :class="{ 'invalid-input': !split.name.trim() }"
              :placeholder="$t('placeholder_split_name')"
              v-model="split.name">
            <button class="template-name-clear" @click="clearField(split, 'name', splitNameInputs.get(`${group.id}-${splitIndex}`) ?? null)">⌫</button>
            <input
              :ref="(el) => { if (el) splitCodeInputs.set(`${group.id}-${splitIndex}`, el as HTMLInputElement) }"
              type="text"
              :class="{ 'invalid-input': !split.code.trim() }"
              :placeholder="$t('placeholder_split_code')"
              v-model="split.code">
            <button class="template-name-clear" @click="clearField(split, 'code', splitCodeInputs.get(`${group.id}-${splitIndex}`) ?? null)">⌫</button>
            <button class="split-delete" @click="deleteSplit(group.id, splitIndex)">
              <img :src="imgGarbage" width="25px" height="25px">
            </button>
          </div>
  
          <div class="template-split">
            <button class="split-add" @click="addSplit(group.id)" :data-tooltip="$t('add_split')">
              <img :src="imgPlus" width="28px" height="28px">
            </button>
          </div>
  
          <div class="template-finish">
            <input
              :ref="(el) => { if (el) finishNameInputs.set(group.id, el as HTMLInputElement) }"
              type="text"
              :class="{ 'invalid-input': !group.finishName }"
              :placeholder="$t('placeholder_finish_name')"
              v-model="group.finishName">
            <button class="template-name-clear" @click="clearField(group, 'finishName', finishNameInputs.get(group.id) ?? null)">⌫</button>
            <input
              :ref="(el) => { if (el) finishCodeInputs.set(group.id, el as HTMLInputElement) }"
              type="text"
              :class="{ 'invalid-input': !group.finishCode }"
              :placeholder="$t('placeholder_finish_code')"
              v-model="group.finishCode">
            <button class="template-name-clear" @click="clearField(group, 'finishCode', finishCodeInputs.get(group.id) ?? null)">⌫</button>
          </div>
        </div>
        <button
          v-if="groups.length > 1"
          class="delete-group"
          @click="deleteGroup(group.id)"
        >
          <img :src="imgGarbage" width="30px" height="30px">
        </button>
      </div>
  
      <div class="ba-group">
        <button v-if="canAddGroup" class="add-group" @click="addGroup" :data-tooltip="$t('add_group')">
          <img :src="imgPlus" width="28px" height="28px">
        </button>
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
.template-builder{
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
}

.template-settings, .template-splits > div{
  display: flex;
  flex-direction: row;
  text-wrap: nowrap;
  gap: 10px;
}
.template-settings{
  padding: 5px 0px 5px 10px;
  z-index: 10;
  flex-wrap: wrap;
  box-shadow: 3px 3px 0 0 rgba(0,0,0,0.5); 
}
.template-settings > *{
  height: 30px;
}
.template-name-clear{
  margin-left: -10px;
}

.template-build{
  display: flex;
  flex-direction: column;
  gap: 20px;
  width: 100%;
  height: 100%;
  padding: 10px 0px 10px 10px;
  overflow-y: auto;
}
.template-group{
  display: flex;
  flex-direction: row;
  width: fit-content;
  box-shadow: 0 0 0 2px rgba(0,0,0,0.5);
  background-color: var(--stngs-bg-color);
  transition: background-color 0.3s;
}
.template-splits{
  display: flex;
  flex-direction: column;
  width: fit-content;
  gap: 10px;
  padding: 10px 10px 10px 0px;
}
.template-split:has(> .split-delete:hover) {
  background-color: rgba(255, 0, 0, 0.3);
  box-shadow: 0 0 0 5px rgba(255, 0, 0, 0.3);
  transition: background-color 0.3s, box-shadow 0.3s;
}
.template-split:has(> .split-drag-handle:hover) {
  box-shadow: 0 0 0 1px rgba(0, 220, 100, 0.3);
  transition: box-shadow 0.3s;
}
.template-splits > div{
  height: 30px;
}
.template-name,
.template-start > input,
.template-split > input,
.template-finish > input{
  text-align: center;
}
.delete-group {
  width: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
}
.template-group:has(> .delete-group:hover) {
  background-color: rgba(255, 0, 0, 0.3);
  transition: background-color 0.3s;
}
.template-group:has(> .group-drag-handle:hover) {
  box-shadow: 0 0 0 1px rgba(0, 220, 100, 0.3);
  transition: box-shadow 0.3s;
}

.ba-group{
  width: fit-content;
}
.add-group{
  height: 30px;
  position: relative;
}

.invalid-input {
  box-shadow: 1px 1px 0 0px rgb(255, 0, 0);
}
.disabled-button {
  opacity: 0.5;
  cursor: default !important;
}

.group-drag-handle {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  padding: 0px 5px;
  cursor: grab;
  color: rgba(255,255,255,0.3);
  user-select: none;
  flex-shrink: 0;
  transition: color 0.15s;
}
.group-drag-handle:hover {
  color: rgba(255,255,255,0.8);
}
.group-drag-handle:active {
  cursor: grabbing;
}

.split-drag-handle {
  cursor: grab;
  color: rgba(255,255,255,0.3);
  user-select: none;
  padding: 0px 5px;
  margin-right: -10px;
  flex-shrink: 0;
  width: 25px;
  transition: color 0.15s;
  align-content: center;
  height: 100%;
}
.split-drag-handle:hover {
  color: rgba(255,255,255,0.8);
}
.split-drag-handle:active {
  cursor: grabbing;
}

.drag-source {
  opacity: 0.3;
}

.template-split.drag-swap-target {
  box-shadow: 0 0 0 2px rgba(0, 220, 100, 0.9) !important;
  background-color: rgba(0, 220, 100, 0.07) !important;
}

.template-group.drag-swap-target {
  box-shadow: 0 0 0 2px rgba(0, 220, 100, 0.9) !important;
  background-color: rgba(0, 220, 100, 0.07) !important;
}

.add-group,
.split-add,
.split-delete{
  background-color: transparent;
}

.split-add{
  position: relative;
}
.split-add[data-tooltip]:hover::after {
  content: attr(data-tooltip);
  position: absolute;
  left: 100%;
  top: 50%;
  transform: translateY(-50%);
  margin-left: 8px;
  padding: 4px 8px;
  background-color: rgba(0, 255, 0, 0.3);
}

.add-group[data-tooltip]:hover::after {
  content: attr(data-tooltip);
  position: absolute;
  left: 100%;
  top: 50%;
  transform: translateY(-50%);
  margin-left: 8px;
  padding: 4px 8px;
  background-color: rgba(0, 255, 0, 0.3);
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
.validation-wrapper{
  display: contents;
}
</style>