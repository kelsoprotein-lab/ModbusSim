<script setup lang="ts">
import { ref, inject, watch, onMounted, onUnmounted, computed, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { ScanGroupInfo, RegisterValueDto, PollDataPayload, ReadResultDto } from '../types'

const emit = defineEmits<{
  (e: 'register-select', regs: RegisterValueDto[]): void
}>()

const selectedConnectionId = inject<Ref<string | null>>('selectedConnectionId')!
const selectedScanGroup = inject<Ref<ScanGroupInfo | null>>('selectedScanGroup')!
const addrMode = inject<Ref<'hex' | 'dec'>>('addrMode')!

const values = ref<RegisterValueDto[]>([])
const selectedIndices = ref<Set<number>>(new Set())
const lastClickedIndex = ref<number>(-1)
const searchFilter = ref('')
const lastUpdated = ref('')
const errorMsg = ref('')

let unlisten: (() => void) | null = null
let unlistenError: (() => void) | null = null
let pollTimer: number | null = null

// Subscribe to poll data events
async function setupListeners() {
  if (unlisten) { unlisten(); unlisten = null }
  if (unlistenError) { unlistenError(); unlistenError = null }

  unlisten = await listen<PollDataPayload>('master-poll-data', (event) => {
    const payload = event.payload
    if (
      payload.connection_id === selectedConnectionId.value &&
      selectedScanGroup.value &&
      payload.scan_group_id === selectedScanGroup.value.id
    ) {
      values.value = payload.result.values
      lastUpdated.value = payload.result.timestamp
      errorMsg.value = ''
      emitSelection()
    }
  })

  unlistenError = await listen<{ connection_id: string; scan_group_id: string; error: string }>('master-poll-error', (event) => {
    const payload = event.payload
    if (
      payload.connection_id === selectedConnectionId.value &&
      selectedScanGroup.value &&
      payload.scan_group_id === selectedScanGroup.value.id
    ) {
      errorMsg.value = payload.error
    }
  })
}

// Timer-based fallback: periodically fetch cached data
function startPollTimer() {
  stopPollTimer()
  pollTimer = window.setInterval(async () => {
    await fetchCachedData()
  }, 2000)
}

function stopPollTimer() {
  if (pollTimer) {
    clearInterval(pollTimer)
    pollTimer = null
  }
}

async function fetchCachedData() {
  if (!selectedConnectionId.value || !selectedScanGroup.value) return
  try {
    const data = await invoke<ReadResultDto | null>('get_poll_data', {
      connectionId: selectedConnectionId.value,
      groupId: selectedScanGroup.value.id,
    })
    if (data && data.values.length > 0) {
      values.value = data.values
      lastUpdated.value = data.timestamp
      errorMsg.value = ''
    }
  } catch (_e) {
    // ignore fetch errors
  }
}

onMounted(async () => {
  await setupListeners()
})

onUnmounted(() => {
  unlisten?.()
  unlistenError?.()
  stopPollTimer()
})

// When scan group changes, reload data + restart timer
watch([selectedConnectionId, selectedScanGroup], async () => {
  selectedIndices.value.clear()
  values.value = []
  lastUpdated.value = ''
  errorMsg.value = ''
  emit('register-select', [])
  stopPollTimer()

  if (!selectedConnectionId.value || !selectedScanGroup.value) return

  // Immediately fetch cached data
  await fetchCachedData()

  // Start periodic refresh as fallback
  startPollTimer()
}, { immediate: true })

// Filtered values
const filteredValues = computed(() => {
  if (!searchFilter.value) return values.value
  const q = searchFilter.value.toLowerCase()
  return values.value.filter(v => {
    const hexAddr = '0x' + v.address.toString(16).toUpperCase().padStart(4, '0')
    return hexAddr.toLowerCase().includes(q) ||
      v.address.toString().includes(q) ||
      v.display_value.toLowerCase().includes(q)
  })
})

function fmtAddress(addr: number): string {
  if (addrMode.value === 'dec') return addr.toString()
  return '0x' + addr.toString(16).toUpperCase().padStart(4, '0')
}

function toggleAddrMode() {
  addrMode.value = addrMode.value === 'hex' ? 'dec' : 'hex'
}

function handleRowClick(index: number, event: MouseEvent) {
  if (event.ctrlKey || event.metaKey) {
    if (selectedIndices.value.has(index)) {
      selectedIndices.value.delete(index)
    } else {
      selectedIndices.value.add(index)
    }
  } else if (event.shiftKey && lastClickedIndex.value >= 0) {
    const start = Math.min(lastClickedIndex.value, index)
    const end = Math.max(lastClickedIndex.value, index)
    for (let i = start; i <= end; i++) {
      selectedIndices.value.add(i)
    }
  } else {
    selectedIndices.value.clear()
    selectedIndices.value.add(index)
  }
  lastClickedIndex.value = index
  emitSelection()
}

function emitSelection() {
  const selected = filteredValues.value.filter((_, i) => selectedIndices.value.has(i))
  emit('register-select', selected)
}

const fcLabel = computed(() => {
  if (!selectedScanGroup.value) return ''
  const map: Record<string, string> = {
    read_coils: '0x Coils',
    read_discrete_inputs: '1x Discrete Inputs',
    read_holding_registers: '3x Holding Registers',
    read_input_registers: '4x Input Registers',
  }
  return map[selectedScanGroup.value.function] || selectedScanGroup.value.function
})
</script>

<template>
  <div class="data-table-container">
    <div v-if="!selectedScanGroup" class="empty-state">
      选择一个扫描组查看数据
    </div>

    <template v-else>
      <div class="table-header">
        <span class="header-title">{{ selectedScanGroup.name }} - {{ fcLabel }}</span>
        <span v-if="errorMsg" class="error-badge" :title="errorMsg">ERR</span>
        <input
          v-model="searchFilter"
          class="search-input"
          type="text"
          placeholder="搜索地址..."
        />
        <button class="mode-btn" @click="toggleAddrMode">{{ addrMode === 'hex' ? 'HEX' : 'DEC' }}</button>
        <span class="register-count">{{ filteredValues.length }} 个</span>
      </div>

      <div v-if="values.length === 0 && !errorMsg" class="empty-state">
        <div>
          <div>暂无数据</div>
          <div class="empty-hint">请确保已启动轮询</div>
        </div>
      </div>

      <div v-else-if="errorMsg && values.length === 0" class="empty-state">
        <div>
          <div class="error-text">{{ errorMsg }}</div>
        </div>
      </div>

      <div v-else class="table-scroll">
        <table class="table">
          <thead>
            <tr>
              <th class="col-addr">地址</th>
              <th class="col-raw">原始值</th>
              <th class="col-display">显示值</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="(val, index) in filteredValues"
              :key="val.address"
              :class="{ selected: selectedIndices.has(index) }"
              @click="handleRowClick(index, $event)"
            >
              <td class="col-addr">{{ fmtAddress(val.address) }}</td>
              <td class="col-raw">
                <template v-if="val.is_bool">
                  <span :class="['bool-value', val.raw_value !== 0 ? 'on' : 'off']">
                    {{ val.raw_value !== 0 ? 'ON' : 'OFF' }}
                  </span>
                </template>
                <template v-else>{{ val.raw_value }}</template>
              </td>
              <td class="col-display">{{ val.display_value }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </template>
  </div>
</template>

<style scoped>
.data-table-container {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: #6c7086;
  font-size: 13px;
  text-align: center;
}

.empty-hint {
  font-size: 11px;
  color: #45475a;
  margin-top: 6px;
}

.error-text {
  color: #f38ba8;
  font-size: 12px;
}

.error-badge {
  font-size: 10px;
  background: #f38ba8;
  color: #1e1e2e;
  padding: 1px 6px;
  border-radius: 3px;
  font-weight: 600;
  cursor: help;
}

.table-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-bottom: 1px solid #313244;
  flex-shrink: 0;
  background: #1e1e2e;
}

.header-title {
  font-size: 12px;
  font-weight: 600;
  color: #89b4fa;
  white-space: nowrap;
}

.search-input {
  flex: 1;
  max-width: 200px;
  padding: 3px 8px;
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  font-size: 12px;
  margin-left: auto;
}

.search-input:focus {
  outline: none;
  border-color: #89b4fa;
}

.mode-btn {
  padding: 2px 8px;
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  cursor: pointer;
  font-size: 11px;
  font-family: monospace;
}

.mode-btn:hover {
  background: #45475a;
}

.register-count {
  font-size: 11px;
  color: #6c7086;
  white-space: nowrap;
}

.table-scroll {
  flex: 1;
  overflow-y: auto;
}

.table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}

.table th {
  position: sticky;
  top: 0;
  background: #1e1e2e;
  color: #6c7086;
  font-weight: 500;
  padding: 6px 10px;
  text-align: left;
  border-bottom: 1px solid #313244;
  z-index: 1;
}

.table tbody tr {
  cursor: pointer;
}

.table tbody tr:hover {
  background: #1e1e2e;
}

.table tbody tr.selected {
  background: #89b4fa;
  color: #1e1e2e;
}

.table td {
  padding: 4px 10px;
  border-bottom: 1px solid #1e1e2e;
}

.col-addr {
  font-family: 'SF Mono', 'Fira Code', monospace;
  width: 90px;
  color: #89b4fa;
}

.table tbody tr.selected .col-addr {
  color: #1e1e2e;
}

.col-raw {
  font-family: monospace;
  width: 100px;
}

.col-display {
  font-family: monospace;
}

.bool-value {
  display: inline-block;
  padding: 1px 8px;
  border-radius: 3px;
  font-size: 11px;
  font-weight: 600;
}

.bool-value.on {
  background: #a6e3a1;
  color: #1e1e2e;
}

.bool-value.off {
  background: #45475a;
  color: #6c7086;
}
</style>
