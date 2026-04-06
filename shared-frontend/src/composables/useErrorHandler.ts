import { ref, readonly } from 'vue'

export interface Toast {
  id: number
  message: string
  level: 'error' | 'warning' | 'info'
  persistent: boolean
  timestamp: number
}

const toasts = ref<Toast[]>([])
let nextId = 0

function classifyError(errorString: string): { level: Toast['level']; persistent: boolean } {
  // Try JSON parse for structured ModbusError
  try {
    const parsed = JSON.parse(errorString)
    if (parsed.category) {
      const cat = parsed.category as string
      if (cat.includes('connection') || cat.includes('serial_port') || cat.includes('timeout')) {
        return { level: 'error', persistent: true }
      }
      if (['illegal_function','illegal_data_address','illegal_data_value','response_timeout','crc_mismatch','lrc_mismatch','frame_error','slave_device_failure'].includes(cat)) {
        return { level: 'warning', persistent: false }
      }
      return { level: 'info', persistent: false }
    }
  } catch { /* not JSON */ }

  // Heuristic for string errors
  const lower = errorString.toLowerCase()
  if (lower.includes('connection') || lower.includes('refused') || lower.includes('timeout')) {
    return { level: 'error', persistent: true }
  }
  return { level: 'warning', persistent: false }
}

export function useErrorHandler() {
  function handleError(error: unknown) {
    const msg = String(error)
    const { level, persistent } = classifyError(msg)
    const id = nextId++
    toasts.value.push({ id, message: msg, level, persistent, timestamp: Date.now() })
    if (!persistent) {
      setTimeout(() => removeToast(id), 3000)
    }
  }

  function removeToast(id: number) {
    toasts.value = toasts.value.filter(t => t.id !== id)
  }

  function clearToasts() {
    toasts.value = []
  }

  return { toasts: readonly(toasts), handleError, removeToast, clearToasts }
}
