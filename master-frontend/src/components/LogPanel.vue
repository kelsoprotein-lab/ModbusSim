<script setup lang="ts">
import { ref, inject, onMounted, watch, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { LogPanelShell, useFcLabel, type LogPanelDataSource, type LogEntry } from 'shared-frontend'
import type { MasterConnectionInfo } from '../types'

interface Props { expanded: boolean }
defineProps<Props>()
defineEmits<{ (e: 'toggle'): void }>()

const selectedConnectionId = inject<Ref<string | null>>('selectedConnectionId')!
const treeRefreshKey = inject<Ref<number>>('treeRefreshKey')!

const connections = ref<{ id: string; label: string }[]>([])

async function loadConnections() {
  try {
    const conns = await invoke<MasterConnectionInfo[]>('list_master_connections')
    connections.value = conns.map(c => ({ id: c.id, label: `${c.target_address}:${c.port}` }))
  } catch { /* ignore */ }
}

onMounted(loadConnections)
watch(treeRefreshKey, loadConnections)

const source: LogPanelDataSource = {
  fetchLogs: (id) => invoke<LogEntry[]>('get_communication_logs', { connectionId: id }),
  clearLogs: (id) => invoke<void>('clear_communication_logs', { connectionId: id }),
  exportCsv: (id) => invoke<string>('export_logs_csv', { connectionId: id }),
}

const { fcLabel } = useFcLabel()
</script>

<template>
  <LogPanelShell
    :expanded="expanded"
    :connections="connections"
    :source="source"
    export-prefix="modbus_master_log"
    :pinned-connection-id="selectedConnectionId"
    :fc-formatter="fcLabel"
    @toggle="$emit('toggle')"
  />
</template>
