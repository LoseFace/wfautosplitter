<script setup lang="ts">
import { ref } from 'vue'
import TemplateList from './components/TemplateList.vue'
import TemplateBuilder from './components/TemplateBuilder.vue'

const mode = ref<'list' | 'builder'>('list')
const selectedTemplate = ref<any>(null)

const openCreate = () => {
  selectedTemplate.value = null
  mode.value = 'builder'
}

const openEdit = (template: any) => {
  selectedTemplate.value = template
  mode.value = 'builder'
}

const goBack = () => {
  mode.value = 'list'
}
</script>

<template>
  <TemplateList
    v-if="mode === 'list'"
    @create="openCreate"
    @edit="openEdit"
  />

  <TemplateBuilder
    v-else
    :templateData="selectedTemplate"
    @back="goBack"
  />
</template>