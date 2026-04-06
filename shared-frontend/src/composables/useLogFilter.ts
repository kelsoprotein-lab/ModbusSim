import { ref, computed, type Ref } from 'vue'
import type { LogEntry } from '../types/modbus'

export type DirectionFilter = 'all' | 'rx' | 'tx'
export type FcFilter = 'all' | string

export function useLogFilter(logs: Ref<LogEntry[]>) {
  const searchQuery = ref('')
  const directionFilter = ref<DirectionFilter>('all')
  const fcFilter = ref<FcFilter>('all')

  const filteredLogs = computed(() => {
    let result = logs.value
    if (directionFilter.value !== 'all') {
      const dir = directionFilter.value.toUpperCase()
      result = result.filter(log => (log.direction || '').toUpperCase() === dir)
    }
    if (fcFilter.value !== 'all') {
      result = result.filter(log => {
        const fc = (log.function_code || '').toUpperCase()
        return fc === fcFilter.value.toUpperCase() || fc.includes(fcFilter.value.toUpperCase())
      })
    }
    if (searchQuery.value.trim()) {
      const q = searchQuery.value.toLowerCase()
      result = result.filter(log =>
        (log.detail || '').toLowerCase().includes(q) ||
        (log.function_code || '').toLowerCase().includes(q) ||
        (log.timestamp || '').toLowerCase().includes(q)
      )
    }
    return result
  })

  const availableFcs = computed(() => {
    const fcs = new Set<string>()
    logs.value.forEach(log => { if (log.function_code) fcs.add(log.function_code) })
    return Array.from(fcs).sort()
  })

  const filterSummary = computed(() => {
    const parts: string[] = []
    if (directionFilter.value !== 'all') parts.push(directionFilter.value.toUpperCase())
    if (fcFilter.value !== 'all') parts.push(fcFilter.value)
    if (searchQuery.value.trim()) parts.push(`"${searchQuery.value}"`)
    return parts.length > 0 ? parts.join(' + ') : ''
  })

  function resetFilters() {
    searchQuery.value = ''
    directionFilter.value = 'all'
    fcFilter.value = 'all'
  }

  return { searchQuery, directionFilter, fcFilter, filteredLogs, availableFcs, filterSummary, resetFilters }
}
