import { ref } from 'vue'
import type { LogEntry } from '../types/modbus'

export interface LogPanelDataSource {
  fetchLogs: (connectionId: string) => Promise<LogEntry[]>
  clearLogs: (connectionId: string) => Promise<void>
  exportCsv: (connectionId: string) => Promise<string>
}

/**
 * Shared log panel logic. Caller injects data source so this composable
 * does not depend on any specific Tauri command names.
 */
export function useLogPanel(source: LogPanelDataSource) {
  const logs = ref<LogEntry[]>([])
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  async function loadLogs(connectionId: string) {
    if (!connectionId) return
    isLoading.value = true
    try {
      logs.value = await source.fetchLogs(connectionId)
    } catch (e) {
      error.value = String(e)
    }
    isLoading.value = false
  }

  async function clearLogs(connectionId: string) {
    if (!connectionId) return
    try {
      await source.clearLogs(connectionId)
      logs.value = []
    } catch (e) {
      error.value = String(e)
    }
  }

  async function exportLogsCsv(connectionId: string, filenamePrefix = 'modbus_log') {
    if (!connectionId) return
    try {
      const csv = await source.exportCsv(connectionId)
      const blob = new Blob([csv], { type: 'text/csv' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `${filenamePrefix}_${Date.now()}.csv`
      a.click()
      URL.revokeObjectURL(url)
    } catch (e) {
      error.value = String(e)
    }
  }

  return { logs, isLoading, error, loadLogs, clearLogs, exportLogsCsv }
}
