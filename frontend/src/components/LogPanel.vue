<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useLogPanel, useLogFilter, useI18n } from 'shared-frontend'

const { t } = useI18n()

interface Props {
  expanded: boolean
}

const props = defineProps<Props>()
const emit = defineEmits<{
  (e: 'toggle'): void
}>()

const { logs, isLoading, error, loadLogs, clearLogs, exportLogsCsv } = useLogPanel()
const { searchQuery, directionFilter, fcFilter, filteredLogs, availableFcs, filterSummary } = useLogFilter(logs)

const connectionList = ref<{ id: string; label: string }[]>([])
const selectedConnId = ref('')
let refreshTimer: number | null = null

async function loadConnections() {
  try {
    const slaves = await invoke<{ id: string; bind_address: string; port: number }[]>('list_slave_connections')
    connectionList.value = slaves.map(s => ({ id: s.id, label: `${s.bind_address}:${s.port}` }))
    if (connectionList.value.length > 0 && !selectedConnId.value) {
      selectedConnId.value = connectionList.value[0].id
    }
  } catch (e) {
    error.value = String(e)
  }
}

async function doLoadLogs() {
  await loadLogs(selectedConnId.value)
}

async function doClearLogs() {
  await clearLogs(selectedConnId.value)
}

async function doExportLogs() {
  await exportLogsCsv(selectedConnId.value, 'modbus_slave_log')
}

function formatTimestamp(ts: string): string {
  try {
    const date = new Date(ts)
    return date.toLocaleTimeString()
  } catch {
    return ts
  }
}

function toggleExpanded() {
  emit('toggle')
}

function startAutoRefresh() {
  if (refreshTimer) return
  refreshTimer = window.setInterval(() => {
    if (props.expanded) {
      doLoadLogs()
    }
  }, 2000)
}

function stopAutoRefresh() {
  if (refreshTimer) {
    clearInterval(refreshTimer)
    refreshTimer = null
  }
}

watch(() => props.expanded, async (expanded) => {
  if (expanded) {
    await loadConnections()
    if (selectedConnId.value) await doLoadLogs()
    startAutoRefresh()
  } else {
    stopAutoRefresh()
  }
})

watch(selectedConnId, () => doLoadLogs())

onMounted(async () => {
  await loadConnections()
  if (selectedConnId.value) {
    await doLoadLogs()
  }
  if (props.expanded) {
    startAutoRefresh()
  }
})
</script>

<template>
  <div :class="['log-panel', { expanded }]">
    <div class="log-header" @click="toggleExpanded">
      <span class="log-toggle">{{ expanded ? '▼' : '▲' }}</span>
      <span class="log-title">{{ t('log.title') }}</span>
      <div class="log-controls" @click.stop>
        <select v-model="selectedConnId" class="conn-select" @change="doLoadLogs">
          <option v-for="conn in connectionList" :key="conn.id" :value="conn.id">
            {{ conn.label }}
          </option>
        </select>
        <button class="log-btn" @click="doLoadLogs" :title="t('common.refresh')">{{ t('common.refresh') }}</button>
        <button class="log-btn" @click="doClearLogs" :title="t('common.clear')">{{ t('common.clear') }}</button>
        <button class="log-btn" @click="doExportLogs" :title="t('common.export')">{{ t('common.export') }}</button>
      </div>
    </div>

    <div v-if="expanded" class="log-filters">
      <input v-model="searchQuery" type="text" class="filter-input" :placeholder="t('log.searchPlaceholder')" />
      <select v-model="directionFilter" class="filter-select">
        <option value="all">{{ t('common.all') }}</option>
        <option value="rx">RX</option>
        <option value="tx">TX</option>
      </select>
      <select v-model="fcFilter" class="filter-select">
        <option value="all">{{ t('common.allFc') }}</option>
        <option v-for="fc in availableFcs" :key="fc" :value="fc">{{ fc }}</option>
      </select>
      <span v-if="filterSummary" class="filter-summary">{{ filterSummary }}</span>
    </div>

    <div v-if="expanded" class="log-body">
      <div v-if="isLoading" class="log-loading">{{ t('common.loading') }}</div>
      <div v-else-if="filteredLogs.length === 0" class="log-empty">{{ t('log.noLogs') }}</div>
      <table v-else class="log-table">
        <thead>
          <tr>
            <th>{{ t('log.timestamp') }}</th>
            <th>{{ t('log.direction') }}</th>
            <th>{{ t('table.function') }}</th>
            <th>{{ t('log.detail') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(log, idx) in filteredLogs" :key="idx">
            <td class="col-time">{{ formatTimestamp(log.timestamp) }}</td>
            <td :class="['col-dir', log.direction.toLowerCase()]">{{ log.direction }}</td>
            <td class="col-func">{{ log.function_code }}</td>
            <td class="col-detail">{{ log.detail }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.log-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  transition: height 0.2s ease;
}

.log-panel:not(.expanded) {
  height: 32px;
}

.log-header {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 32px;
  padding: 0 8px;
  cursor: pointer;
  flex-shrink: 0;
  background: #1e1e2e;
}

.log-toggle {
  font-size: 10px;
  color: #6c7086;
  width: 16px;
  text-align: center;
}

.log-title {
  font-size: 12px;
  color: #6c7086;
}

.log-controls {
  display: flex;
  gap: 4px;
  margin-left: auto;
}

.conn-select {
  padding: 2px 6px;
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  font-size: 11px;
  max-width: 140px;
}

.log-btn {
  padding: 2px 8px;
  background: transparent;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  cursor: pointer;
  font-size: 11px;
}

.log-btn:hover {
  background: #313244;
}

.log-body {
  flex: 1;
  overflow-y: auto;
  background: #11111b;
}

.log-loading,
.log-empty {
  padding: 24px;
  text-align: center;
  color: #6c7086;
  font-size: 12px;
}

.log-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}

.log-table th,
.log-table td {
  padding: 4px 10px;
  text-align: left;
  border-bottom: 1px solid #1e1e2e;
}

.log-table th {
  background: #181825;
  color: #6c7086;
  font-weight: 500;
  position: sticky;
  top: 0;
}

.col-time {
  font-family: monospace;
  color: #6c7086;
  width: 80px;
}

.col-dir {
  font-weight: 600;
  width: 40px;
}

.col-dir.rx {
  color: #89b4fa;
}

.col-dir.tx {
  color: #a6e3a1;
}

.col-func {
  font-family: monospace;
  width: 70px;
}

.col-detail {
  font-family: monospace;
}

.log-filters {
  display: flex;
  gap: 6px;
  padding: 4px 8px;
  align-items: center;
  border-bottom: 1px solid #313244;
}
.filter-input {
  flex: 1;
  padding: 3px 8px;
  background: #11111b;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  font-size: 12px;
}
.filter-input:focus { outline: none; border-color: #89b4fa; }
.filter-select {
  padding: 3px 6px;
  background: #11111b;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  font-size: 12px;
}
.filter-summary {
  font-size: 11px;
  color: #89b4fa;
  white-space: nowrap;
}
</style>
