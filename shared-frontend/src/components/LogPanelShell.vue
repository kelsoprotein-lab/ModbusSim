<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from '../i18n'
import { useLogPanel, type LogPanelDataSource } from '../composables/useLogPanel'
import { useLogFilter } from '../composables/useLogFilter'

interface ConnectionItem {
  id: string
  label: string
}

interface LogAppendedEvent {
  connection_id: string
}

interface Props {
  expanded: boolean
  /** Always-up-to-date list of connections from the parent. */
  connections: ConnectionItem[]
  /** Backend command bindings — see LogPanelDataSource. */
  source: LogPanelDataSource
  /** Filename prefix for the CSV export. */
  exportPrefix?: string
  /** When set, will auto-select this id when present in `connections`. */
  pinnedConnectionId?: string | null
  /** Optional formatter, e.g. for mapping `read_holding_registers` → `FC03`. */
  fcFormatter?: (fc: string) => string
  /** Optional override for timestamp formatting. */
  timestampFormatter?: (ts: string) => string
}

const props = withDefaults(defineProps<Props>(), {
  exportPrefix: 'modbus_log',
  pinnedConnectionId: null,
  fcFormatter: (fc: string) => fc,
  timestampFormatter: undefined,
})

const emit = defineEmits<{ (e: 'toggle'): void }>()

const { t, locale } = useI18n()
const { logs, isLoading, error, loadLogs, clearLogs, exportLogsCsv } = useLogPanel(props.source)
const { searchQuery, directionFilter, fcFilter, filteredLogs, availableFcs, filterSummary } = useLogFilter(logs)

const selectedConnId = ref('')
let unlisten: UnlistenFn | null = null
let reloadInFlight = false

function pickInitialConnection() {
  const list = props.connections
  if (props.pinnedConnectionId && list.some(c => c.id === props.pinnedConnectionId)) {
    selectedConnId.value = props.pinnedConnectionId
    return
  }
  if (list.length > 0 && !list.some(c => c.id === selectedConnId.value)) {
    selectedConnId.value = list[0].id
  }
}

/** Coalesce append events: drop additional events while a fetch is in flight.
 *  Each fetch returns the full buffer, so a single follow-up always catches
 *  up — no need to chain a second fetch. */
async function scheduleReload() {
  if (reloadInFlight) return
  if (!props.expanded || !selectedConnId.value) return
  reloadInFlight = true
  try { await loadLogs(selectedConnId.value) } finally { reloadInFlight = false }
}

async function doLoadLogs() {
  if (!selectedConnId.value) return
  await loadLogs(selectedConnId.value)
}

async function doClearLogs() {
  if (!selectedConnId.value) return
  await clearLogs(selectedConnId.value)
}

async function doExportLogs() {
  if (!selectedConnId.value) return
  await exportLogsCsv(selectedConnId.value, props.exportPrefix)
}

function fmtTimestamp(ts: string): string {
  if (props.timestampFormatter) return props.timestampFormatter(ts)
  const date = new Date(ts)
  if (isNaN(date.getTime())) return ts
  return date.toLocaleTimeString(locale.value, {
    hour12: false,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

async function startListening() {
  if (unlisten) return
  unlisten = await listen<LogAppendedEvent>('log-appended', (event) => {
    if (event.payload.connection_id === selectedConnId.value) {
      scheduleReload()
    }
  })
}

function stopListening() {
  if (unlisten) {
    unlisten()
    unlisten = null
  }
}

watch(() => props.connections, pickInitialConnection, { deep: false })

watch(() => props.pinnedConnectionId, (id) => {
  if (id && props.connections.some(c => c.id === id)) {
    selectedConnId.value = id
  }
})

watch(selectedConnId, () => doLoadLogs())

watch(() => props.expanded, async (expanded) => {
  if (expanded) {
    pickInitialConnection()
    await doLoadLogs()
  }
})

onMounted(async () => {
  await startListening()
  pickInitialConnection()
  await doLoadLogs()
})

onUnmounted(stopListening)
</script>

<template>
  <div :class="['log-panel', { expanded }]">
    <div class="log-header" @click="emit('toggle')">
      <span class="log-toggle">{{ expanded ? '▼' : '▲' }}</span>
      <span class="log-title">{{ t('log.title') }}</span>
      <span v-if="!expanded && logs.length > 0" class="log-count">{{ logs.length }}</span>
      <div class="log-controls" @click.stop>
        <select v-model="selectedConnId" class="conn-select">
          <option v-for="conn in connections" :key="conn.id" :value="conn.id">{{ conn.label }}</option>
        </select>
        <button class="log-btn" @click="doLoadLogs">{{ t('common.refresh') }}</button>
        <button class="log-btn" @click="doClearLogs">{{ t('common.clear') }}</button>
        <button class="log-btn" @click="doExportLogs">{{ t('common.export') }}</button>
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
        <option v-for="fc in availableFcs" :key="fc" :value="fc">{{ fcFormatter(fc) }}</option>
      </select>
      <span v-if="filterSummary" class="filter-summary">{{ filterSummary }}</span>
      <span v-if="error" class="filter-error" :title="error">!</span>
    </div>

    <div v-if="expanded" class="log-body">
      <div v-if="isLoading" class="log-empty">{{ t('common.loading') }}</div>
      <div v-else-if="connections.length === 0" class="log-empty">{{ t('tree.noConnection') }}</div>
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
            <td class="col-time">{{ fmtTimestamp(log.timestamp) }}</td>
            <td :class="['col-dir', (log.direction || '').toLowerCase()]">{{ (log.direction || '').toUpperCase() }}</td>
            <td class="col-func">{{ fcFormatter(log.function_code) }}</td>
            <td class="col-detail">{{ log.detail }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.log-panel { display: flex; flex-direction: column; height: 100%; transition: height 0.2s ease; }
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
.col-func { font-family: monospace; width: 60px; }
.col-detail { font-family: monospace; }
.log-filters { display: flex; gap: 6px; padding: 4px 8px; align-items: center; border-bottom: 1px solid #313244; }
.filter-input { flex: 1; padding: 3px 8px; background: #11111b; border: 1px solid #45475a; border-radius: 4px; color: #cdd6f4; font-size: 12px; }
.filter-input:focus { outline: none; border-color: #89b4fa; }
.filter-select { padding: 3px 6px; background: #11111b; border: 1px solid #45475a; border-radius: 4px; color: #cdd6f4; font-size: 12px; }
.filter-summary { font-size: 11px; color: #89b4fa; white-space: nowrap; }
.filter-error { font-size: 12px; color: #f38ba8; font-weight: 700; cursor: help; }
</style>
