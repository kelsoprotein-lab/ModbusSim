<script setup lang="ts">
import { ref, inject, onMounted, watch, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { LogEntry, MasterConnectionInfo } from '../types'

interface Props {
  expanded: boolean
}

const props = defineProps<Props>()
const emit = defineEmits<{
  (e: 'toggle'): void
}>()

const selectedConnectionId = inject<Ref<string | null>>('selectedConnectionId')!

const logs = ref<LogEntry[]>([])
const connectionList = ref<{ id: string; label: string }[]>([])
const selectedConnId = ref('')
let refreshTimer: number | null = null

async function loadConnections() {
  try {
    const conns = await invoke<MasterConnectionInfo[]>('list_master_connections')
    connectionList.value = conns.map(c => ({
      id: c.id,
      label: `${c.target_address}:${c.port}`,
    }))
    // Auto-select: prefer the currently selected connection in the tree
    if (selectedConnectionId.value && conns.some(c => c.id === selectedConnectionId.value)) {
      selectedConnId.value = selectedConnectionId.value
    } else if (connectionList.value.length > 0 && !selectedConnId.value) {
      selectedConnId.value = connectionList.value[0].id
    }
  } catch (_e) { /* ignore */ }
}

async function loadLogs() {
  if (!selectedConnId.value) return
  try {
    logs.value = await invoke<LogEntry[]>('get_communication_logs', {
      connectionId: selectedConnId.value,
    })
  } catch (_e) { /* ignore */ }
}

async function clearLogs() {
  if (!selectedConnId.value) return
  try {
    await invoke('clear_communication_logs', { connectionId: selectedConnId.value })
    logs.value = []
  } catch (_e) { /* ignore */ }
}

async function exportLogs() {
  if (!selectedConnId.value) return
  try {
    const csv = await invoke<string>('export_logs_csv', { connectionId: selectedConnId.value })
    const blob = new Blob([csv], { type: 'text/csv' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `modbus_master_log_${Date.now()}.csv`
    a.click()
    URL.revokeObjectURL(url)
  } catch (_e) { /* ignore */ }
}

function formatTimestamp(ts: string): string {
  try {
    const date = new Date(ts)
    if (isNaN(date.getTime())) return ts
    return date.toLocaleTimeString('zh-CN', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit', fractionalSecondDigits: 3 } as Intl.DateTimeFormatOptions)
  } catch {
    return ts
  }
}

function formatDirection(dir: string): string {
  return dir.toUpperCase()
}

const fcNameMap: Record<string, string> = {
  read_coils: 'FC01',
  read_discrete_inputs: 'FC02',
  read_holding_registers: 'FC03',
  read_input_registers: 'FC04',
  write_single_coil: 'FC05',
  write_single_register: 'FC06',
  write_multiple_coils: 'FC15',
  write_multiple_registers: 'FC16',
}

function formatFunctionCode(fc: string): string {
  return fcNameMap[fc] || fc
}

function dirClass(dir: string): string {
  return dir.toLowerCase()
}

function startAutoRefresh() {
  if (refreshTimer) return
  refreshTimer = window.setInterval(() => {
    if (props.expanded) {
      loadConnections()
      loadLogs()
    }
  }, 2000)
}

function stopAutoRefresh() {
  if (refreshTimer) {
    clearInterval(refreshTimer)
    refreshTimer = null
  }
}

// When the selected connection in the tree changes, auto-select it in log panel
watch(selectedConnectionId, (newId) => {
  if (newId && connectionList.value.some(c => c.id === newId)) {
    selectedConnId.value = newId
  }
})

watch(() => props.expanded, (expanded) => {
  if (expanded) {
    loadConnections()
    loadLogs()
    startAutoRefresh()
  } else {
    stopAutoRefresh()
  }
})

watch(selectedConnId, () => loadLogs())

onMounted(async () => {
  await loadConnections()
  if (selectedConnId.value) await loadLogs()
  if (props.expanded) startAutoRefresh()
})
</script>

<template>
  <div :class="['log-panel', { expanded }]">
    <div class="log-header" @click="emit('toggle')">
      <span class="log-toggle">{{ expanded ? '▼' : '▲' }}</span>
      <span class="log-title">通信日志</span>
      <span v-if="!expanded && logs.length > 0" class="log-count">{{ logs.length }}</span>
      <div class="log-controls" @click.stop>
        <select v-model="selectedConnId" class="conn-select" @change="loadLogs">
          <option v-for="conn in connectionList" :key="conn.id" :value="conn.id">{{ conn.label }}</option>
        </select>
        <button class="log-btn" @click="loadLogs">刷新</button>
        <button class="log-btn" @click="clearLogs">清空</button>
        <button class="log-btn" @click="exportLogs">导出</button>
      </div>
    </div>

    <div v-if="expanded" class="log-body">
      <div v-if="connectionList.length === 0" class="log-empty">暂无连接</div>
      <div v-else-if="logs.length === 0" class="log-empty">暂无日志</div>
      <table v-else class="log-table">
        <thead>
          <tr>
            <th>时间</th>
            <th>方向</th>
            <th>功能码</th>
            <th>详情</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(log, idx) in logs" :key="idx">
            <td class="col-time">{{ formatTimestamp(log.timestamp) }}</td>
            <td :class="['col-dir', dirClass(log.direction)]">{{ formatDirection(log.direction) }}</td>
            <td class="col-func">{{ formatFunctionCode(log.function_code) }}</td>
            <td class="col-detail">{{ log.detail }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.log-panel { display: flex; flex-direction: column; height: 100%; }
.log-panel:not(.expanded) { height: 32px; }
.log-header { display: flex; align-items: center; gap: 8px; height: 32px; padding: 0 8px; cursor: pointer; flex-shrink: 0; background: #1e1e2e; }
.log-toggle { font-size: 10px; color: #6c7086; width: 16px; text-align: center; }
.log-title { font-size: 12px; color: #6c7086; }
.log-count { font-size: 10px; background: #89b4fa; color: #1e1e2e; padding: 0 6px; border-radius: 8px; font-weight: 600; }
.log-controls { display: flex; gap: 4px; margin-left: auto; }
.conn-select { padding: 2px 6px; background: #313244; border: 1px solid #45475a; border-radius: 4px; color: #cdd6f4; font-size: 11px; max-width: 160px; }
.log-btn { padding: 2px 8px; background: transparent; border: 1px solid #45475a; border-radius: 4px; color: #cdd6f4; cursor: pointer; font-size: 11px; }
.log-btn:hover { background: #313244; }
.log-body { flex: 1; overflow-y: auto; background: #11111b; }
.log-empty { padding: 24px; text-align: center; color: #6c7086; font-size: 12px; }
.log-table { width: 100%; border-collapse: collapse; font-size: 12px; }
.log-table th, .log-table td { padding: 4px 10px; text-align: left; border-bottom: 1px solid #1e1e2e; }
.log-table th { background: #181825; color: #6c7086; font-weight: 500; position: sticky; top: 0; }
.col-time { font-family: monospace; color: #6c7086; width: 100px; }
.col-dir { font-weight: 600; width: 40px; }
.col-dir.rx { color: #89b4fa; }
.col-dir.tx { color: #a6e3a1; }
.col-func { font-family: monospace; width: 50px; }
.col-detail { font-family: monospace; }
</style>
