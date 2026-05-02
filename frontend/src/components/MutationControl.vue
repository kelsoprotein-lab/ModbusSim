<script setup lang="ts">
import { ref, watch, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'shared-frontend'

const { t } = useI18n()

interface Props { connectionId: string | null; slaveId: number | null }
const props = defineProps<Props>()
const emit = defineEmits<{ (e: 'mutated'): void }>()

const active = ref(false)
const intervalMs = ref(1000)
const types = ref<Record<string, boolean>>({
  coil: true,
  discrete_input: false,
  holding_register: true,
  input_register: false,
})
let timer: number | null = null

function toggle() {
  if (active.value) stop()
  else start()
}

function start() {
  if (!props.connectionId || props.slaveId === null) return
  active.value = true
  schedule()
}

function stop() {
  active.value = false
  if (timer !== null) {
    clearTimeout(timer)
    timer = null
  }
}

function schedule() {
  if (!active.value) return
  timer = window.setTimeout(async () => {
    if (!active.value || !props.connectionId || props.slaveId === null) {
      stop()
      return
    }
    const enabledTypes = Object.entries(types.value).filter(([, v]) => v).map(([k]) => k)
    if (enabledTypes.length > 0) {
      try {
        await invoke<number>('random_mutate_registers', {
          request: {
            connection_id: props.connectionId,
            slave_id: props.slaveId,
            register_types: enabledTypes,
          }
        })
        emit('mutated')
      } catch (e) {
        console.error('mutation failed:', e)
      }
    }
    schedule()
  }, intervalMs.value)
}

watch(() => [props.connectionId, props.slaveId], () => {
  if (active.value) stop()
})

onUnmounted(() => {
  if (timer !== null) clearTimeout(timer)
})
</script>

<template>
  <div class="mutation-group">
    <button
      :class="['toolbar-btn', { 'btn-mutation-active': active }]"
      @click="toggle"
      :disabled="!connectionId || slaveId === null"
      :title="t('toolbar.randomMutation')"
    >
      <span class="toolbar-label">{{ active ? t('toolbar.stopMutation') : t('toolbar.randomMutation') }}</span>
    </button>
    <input
      type="range"
      class="rate-slider"
      min="100"
      max="5000"
      step="100"
      v-model.number="intervalMs"
      :title="t('toolbar.mutationInterval')"
    />
    <span class="rate-label">{{ intervalMs }}ms</span>
    <label
      v-for="key in (['coil','discrete_input','holding_register','input_register'] as const)"
      :key="key"
      class="mutation-type-label"
    >
      <input type="checkbox" v-model="types[key]" />
      {{ ({ coil: t('table.coil'), discrete_input: t('table.discreteInput'), holding_register: t('table.holdingRegister'), input_register: t('table.inputRegister') })[key] }}
    </label>
  </div>
</template>

<style scoped>
.mutation-group { display: flex; align-items: center; gap: 4px; }
.toolbar-btn { display: flex; align-items: center; gap: 4px; padding: 4px 10px; border: none; background: transparent; color: #cdd6f4; cursor: pointer; border-radius: 4px; font-size: 12px; white-space: nowrap; }
.toolbar-btn:hover:not(:disabled) { background: #313244; }
.toolbar-btn:disabled { opacity: 0.4; cursor: default; }
.toolbar-btn.btn-mutation-active { background: #a6e3a1; color: #1e1e2e; font-weight: 600; }
.toolbar-btn.btn-mutation-active:hover { background: #94e2d5; }
.rate-slider { width: 80px; height: 4px; accent-color: #89b4fa; cursor: pointer; }
.rate-label { font-size: 10px; color: #6c7086; min-width: 42px; font-family: 'SF Mono', 'Fira Code', monospace; }
.mutation-type-label { display: flex; align-items: center; gap: 3px; font-size: 11px; color: #a6adc8; cursor: pointer; white-space: nowrap; }
.mutation-type-label input[type="checkbox"] { accent-color: #89b4fa; }
</style>
