import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { LogEntry } from '../types/modbus'

/**
 * Shared log panel logic for both slave and master frontends.
 */
export function useLogPanel() {
  const logs = ref<LogEntry[]>([])
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  async function loadLogs(connectionId: string) {
    if (!connectionId) return
    isLoading.value = true
    try {
      logs.value = await invoke<LogEntry[]>('get_communication_logs', {
        connectionId,
      })
    } catch (e) {
      error.value = String(e)
    }
    isLoading.value = false
  }

  async function clearLogs(connectionId: string) {
    if (!connectionId) return
    try {
      await invoke('clear_communication_logs', {
        connectionId,
      })
      logs.value = []
    } catch (e) {
      error.value = String(e)
    }
  }

  async function exportLogsCsv(connectionId: string, filenamePrefix = 'modbus_log') {
    if (!connectionId) return
    try {
      const csv = await invoke<string>('export_logs_csv', {
        connectionId,
      })
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
