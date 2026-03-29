<script setup lang="ts">
import { ref, inject, watch, computed, nextTick, provide, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { dialogKey } from '../composables/useDialog'
import type { showAlert as ShowAlert } from '../composables/useDialog'
import RegisterModal from './RegisterModal.vue'
import BatchAddModal from './BatchAddModal.vue'

const { showAlert } = inject<{ showAlert: typeof ShowAlert }>(dialogKey)!

interface Register {
  address: number
  register_type: string
  data_type: string
  endian: string
  name: string
  comment: string
}

const emit = defineEmits<{
  (e: 'register-select', regs: { address: number; register_type: string; value: number }[]): void
}>()

const selectedConnectionId = inject<Ref<string | null>>('selectedConnectionId')!
const selectedSlaveId = inject<Ref<number | null>>('selectedSlaveId')!
const selectedRegisterType = inject<Ref<string | null>>('selectedRegisterType')!
const registerRefreshKey = inject<Ref<number>>('registerRefreshKey')!

const registers = ref<Register[]>([])
const selectedRows = ref<Register[]>([])
const lastClickedIndex = ref<number>(-1)
const registerValues = ref<Record<string, number>>({})
const editingCell = ref<{ address: number; register_type: string } | null>(null)
const editValue = ref('')
const isLoading = ref(false)
const error = ref<string | null>(null)
const searchQuery = ref('')
const contextMenu = ref({ show: false, x: 0, y: 0, reg: null as Register | null })
const scrollContainer = ref<HTMLDivElement | null>(null)
const addrMode = ref<'hex' | 'dec'>('hex')
provide('addrMode', addrMode)
const showAddModal = ref(false)
const showBatchModal = ref(false)

function onRegisterSaved() {
  registerRefreshKey.value++
}

// Filter registers by selected type + search query
const filteredRegisters = computed(() => {
  let result = registers.value
  if (selectedRegisterType.value) {
    result = result.filter(r => r.register_type === selectedRegisterType.value)
  }
  const q = searchQuery.value.trim()
  if (!q) return result
  if (q.startsWith('0x') || q.startsWith('0X')) {
    const hexPart = q.slice(2).toUpperCase()
    if (!hexPart) return result
    return result.filter(r => {
      const addrHex = r.address.toString(16).toUpperCase().padStart(4, '0')
      return addrHex.includes(hexPart)
    })
  }
  if (/^\d+$/.test(q)) {
    const num = Number(q)
    const hexPart = q.toUpperCase()
    return result.filter(r => {
      if (r.address === num) return true
      const addrHex = r.address.toString(16).toUpperCase().padStart(4, '0')
      return addrHex.includes(hexPart)
    })
  }
  const lower = q.toLowerCase()
  return result.filter(r => r.name.toLowerCase().includes(lower))
})

// Clear selection when search changes
watch(searchQuery, () => {
  clearSelection()
})

async function loadRegisters() {
  if (!selectedConnectionId.value || selectedSlaveId.value === null) {
    registers.value = []
    registerValues.value = {}
    return
  }
  isLoading.value = true
  error.value = null
  try {
    const defs = await invoke<Register[]>('list_registers', {
      connectionId: selectedConnectionId.value,
      slaveId: selectedSlaveId.value,
    })
    registers.value = defs

    const values: Record<string, number> = {}
    for (const reg of defs) {
      const result = await invoke<{ address: number; value: number }>('read_register', {
        connectionId: selectedConnectionId.value,
        slaveId: selectedSlaveId.value,
        registerType: reg.register_type,
        address: reg.address,
      })
      values[`${reg.register_type}-${reg.address}`] = result.value
    }
    registerValues.value = values
  } catch (e) {
    error.value = String(e)
  }
  isLoading.value = false
}

watch([selectedConnectionId, selectedSlaveId, selectedRegisterType], () => {
  clearSelection()
  loadRegisters()
})

watch(registerRefreshKey, async () => {
  if (!selectedConnectionId.value || selectedSlaveId.value === null) return
  for (const reg of registers.value) {
    try {
      const result = await invoke<{ address: number; value: number }>('read_register', {
        connectionId: selectedConnectionId.value,
        slaveId: selectedSlaveId.value,
        registerType: reg.register_type,
        address: reg.address,
      })
      registerValues.value[`${reg.register_type}-${reg.address}`] = result.value
    } catch { /* skip */ }
  }
  emitSelection()
})

function clearSelection() {
  selectedRows.value = []
  lastClickedIndex.value = -1
  emitSelection()
}

function getValue(reg: Register): number {
  return registerValues.value[`${reg.register_type}-${reg.address}`] ?? 0
}

function isSelected(reg: Register): boolean {
  return selectedRows.value.some(r => r.address === reg.address && r.register_type === reg.register_type)
}

function selectRow(e: MouseEvent, reg: Register) {
  const list = filteredRegisters.value
  const idx = list.indexOf(reg)
  const isCtrl = e.ctrlKey || e.metaKey

  if (e.shiftKey && lastClickedIndex.value >= 0) {
    // Shift+click: range select
    const start = Math.min(lastClickedIndex.value, idx)
    const end = Math.max(lastClickedIndex.value, idx)
    selectedRows.value = list.slice(start, end + 1)
  } else if (isCtrl) {
    // Ctrl/Cmd+click: toggle
    if (isSelected(reg)) {
      selectedRows.value = selectedRows.value.filter(r => !(r.address === reg.address && r.register_type === reg.register_type))
    } else {
      selectedRows.value = [...selectedRows.value, reg]
    }
    lastClickedIndex.value = idx
  } else {
    // Normal click: replace
    selectedRows.value = [reg]
    lastClickedIndex.value = idx
  }

  emitSelection()
}

function emitSelection() {
  const regs = selectedRows.value.map(r => ({
    address: r.address,
    register_type: r.register_type,
    value: getValue(r),
  }))
  emit('register-select', regs)
}

function handleTableKeydown(e: KeyboardEvent) {
  // If editing, let the edit input handle keys
  if (editingCell.value) return

  const list = filteredRegisters.value
  if (list.length === 0) return

  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault()
    // Find current position based on the last single-selected row
    let currentIdx = -1
    if (selectedRows.value.length > 0) {
      const last = selectedRows.value[selectedRows.value.length - 1]
      currentIdx = list.findIndex(r => r.address === last.address && r.register_type === last.register_type)
    }

    let nextIdx: number
    if (e.key === 'ArrowDown') {
      nextIdx = currentIdx < list.length - 1 ? currentIdx + 1 : currentIdx
    } else {
      nextIdx = currentIdx > 0 ? currentIdx - 1 : 0
    }

    if (nextIdx >= 0 && nextIdx < list.length) {
      selectedRows.value = [list[nextIdx]]
      lastClickedIndex.value = nextIdx
      emitSelection()

      // Scroll into view
      nextTick(() => {
        const container = scrollContainer.value
        if (!container) return
        const rows = container.querySelectorAll('tbody tr')
        if (rows[nextIdx]) {
          rows[nextIdx].scrollIntoView({ block: 'nearest' })
        }
      })
    }
  }
}

function startEdit(reg: Register) {
  editingCell.value = { address: reg.address, register_type: reg.register_type }
  editValue.value = String(getValue(reg))
}

async function commitEdit() {
  if (!editingCell.value || !selectedConnectionId.value || selectedSlaveId.value === null) return
  const { address, register_type } = editingCell.value
  const value = Number(editValue.value)
  if (isNaN(value)) return

  try {
    await invoke('write_register', {
      request: {
        connection_id: selectedConnectionId.value,
        slave_id: selectedSlaveId.value,
        register_type,
        address,
        value: register_type === 'coil' || register_type === 'discrete_input' ? (value !== 0 ? 1 : 0) : value,
      }
    })
    registerValues.value[`${register_type}-${address}`] = register_type === 'coil' || register_type === 'discrete_input' ? (value !== 0 ? 1 : 0) : value
    emitSelection()
  } catch (e) {
    await showAlert(String(e))
  }
  editingCell.value = null
}

function cancelEdit() {
  editingCell.value = null
}

function handleEditKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter') {
    commitEdit()
  } else if (e.key === 'Escape') {
    cancelEdit()
  }
}

function showContextMenu(e: MouseEvent, reg: Register) {
  e.preventDefault()
  contextMenu.value = { show: true, x: e.clientX, y: e.clientY, reg }
}

function closeContextMenu() {
  contextMenu.value.show = false
}

async function deleteRegister() {
  const reg = contextMenu.value.reg
  contextMenu.value.show = false
  if (!reg || !selectedConnectionId.value || selectedSlaveId.value === null) return
  try {
    await invoke('remove_register', {
      connectionId: selectedConnectionId.value,
      slaveId: selectedSlaveId.value,
      address: reg.address,
      registerType: reg.register_type,
    })
    if (isSelected(reg)) {
      selectedRows.value = selectedRows.value.filter(r => !(r.address === reg.address && r.register_type === reg.register_type))
      emitSelection()
    }
    await loadRegisters()
  } catch (e) {
    await showAlert(String(e))
  }
}

function formatAddress(reg: Register): string {
  if (addrMode.value === 'dec') return reg.address.toString()
  return '0x' + reg.address.toString(16).toUpperCase().padStart(4, '0')
}

function toggleAddrMode() {
  addrMode.value = addrMode.value === 'hex' ? 'dec' : 'hex'
}

function formatRegType(type: string): string {
  const map: Record<string, string> = {
    coil: 'Coil (FC1)',
    discrete_input: 'Discrete Input (FC2)',
    holding_register: 'Holding Register (FC3)',
    input_register: 'Input Register (FC4)',
  }
  return map[type] || type
}
</script>

<template>
  <div class="register-table" @click="closeContextMenu">
    <div class="table-header-bar">
      <span class="table-title">
        {{ selectedRegisterType ? formatRegType(selectedRegisterType) : '全部寄存器' }}
      </span>
      <input
        v-model="searchQuery"
        class="search-input"
        type="text"
        placeholder="搜索地址/名称..."
      />
      <button class="addr-mode-btn" @click="toggleAddrMode" :title="addrMode === 'hex' ? '切换为十进制' : '切换为十六进制'">
        {{ addrMode === 'hex' ? 'HEX' : 'DEC' }}
      </button>
      <button
        class="add-reg-btn"
        :disabled="!selectedConnectionId || selectedSlaveId === null"
        @click="showAddModal = true"
        title="添加寄存器"
      >+</button>
      <button
        class="add-reg-btn batch"
        :disabled="!selectedConnectionId || selectedSlaveId === null"
        @click="showBatchModal = true"
        title="批量添加寄存器"
      >批量</button>
      <span class="table-count">{{ filteredRegisters.length }} 个寄存器</span>
    </div>

    <div v-if="isLoading" class="table-loading">加载中...</div>
    <div v-else-if="!selectedConnectionId || selectedSlaveId === null" class="table-empty">
      请在左侧树形导航中选择一个从站
    </div>
    <div v-else-if="filteredRegisters.length === 0" class="table-empty">
      暂无寄存器
    </div>

    <div
      v-else
      ref="scrollContainer"
      class="table-scroll-container"
      tabindex="0"
      @keydown="handleTableKeydown"
    >
      <table class="table">
        <thead>
          <tr>
            <th>地址</th>
            <th>名称</th>
            <th>值</th>
            <th>备注</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="reg in filteredRegisters"
            :key="`${reg.register_type}-${reg.address}`"
            :class="{ selected: isSelected(reg) }"
            @click="selectRow($event, reg)"
          >
            <td class="col-addr">{{ formatAddress(reg) }}</td>
            <td class="col-name">{{ reg.name || '-' }}</td>
            <td class="col-value" @dblclick.stop="startEdit(reg)">
              <template v-if="editingCell?.address === reg.address && editingCell?.register_type === reg.register_type">
                <input
                  v-model="editValue"
                  class="edit-input"
                  type="number"
                  autofocus
                  @blur="commitEdit"
                  @keydown="handleEditKeydown"
                  @click.stop
                />
              </template>
              <template v-else>
                <span
                  v-if="reg.register_type === 'coil' || reg.register_type === 'discrete_input'"
                  :class="['bool-value', getValue(reg) ? 'on' : 'off']"
                >
                  {{ getValue(reg) ? 'ON' : 'OFF' }}
                </span>
                <span v-else class="num-value">{{ getValue(reg) }}</span>
              </template>
            </td>
            <td class="col-comment" @contextmenu.prevent="showContextMenu($event, reg)">
              {{ reg.comment || '-' }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Context Menu -->
    <div
      v-if="contextMenu.show"
      class="context-menu"
      :style="{ top: contextMenu.y + 'px', left: contextMenu.x + 'px' }"
      @click.stop
    >
      <div class="context-menu-item danger" @click="deleteRegister">删除寄存器</div>
    </div>

    <!-- Add Register Modal -->
    <RegisterModal
      :show="showAddModal"
      mode="add"
      :existing-registers="registers"
      :connection-id="selectedConnectionId ?? ''"
      :slave-id="selectedSlaveId ?? 0"
      @close="showAddModal = false"
      @saved="onRegisterSaved"
    />

    <!-- Batch Add Modal -->
    <BatchAddModal
      :show="showBatchModal"
      :existing-registers="registers"
      :connection-id="selectedConnectionId ?? ''"
      :slave-id="selectedSlaveId ?? 0"
      @close="showBatchModal = false"
      @saved="onRegisterSaved"
    />
  </div>
</template>

<style scoped>
.register-table {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.table-header-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid #313244;
  flex-shrink: 0;
}

.table-title {
  font-size: 12px;
  font-weight: 600;
  color: #cdd6f4;
  white-space: nowrap;
}

.search-input {
  flex: 1;
  min-width: 0;
  padding: 4px 8px;
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  font-size: 12px;
  outline: none;
}

.search-input:focus {
  border-color: #89b4fa;
}

.search-input::placeholder {
  color: #6c7086;
}

.addr-mode-btn {
  padding: 2px 8px;
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  font-size: 11px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  cursor: pointer;
  white-space: nowrap;
}

.addr-mode-btn:hover {
  background: #45475a;
}

.add-reg-btn {
  padding: 2px 8px;
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #a6e3a1;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
  line-height: 1;
}

.add-reg-btn.batch {
  font-size: 11px;
  font-weight: 400;
}

.add-reg-btn:hover:not(:disabled) {
  background: #45475a;
}

.add-reg-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.table-count {
  font-size: 11px;
  color: #6c7086;
  white-space: nowrap;
}

.table-loading,
.table-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #6c7086;
  font-size: 13px;
}

.table-scroll-container {
  flex: 1;
  overflow-y: auto;
  outline: none;
}

.table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}

.table thead {
  position: sticky;
  top: 0;
  z-index: 1;
}

.table th {
  background: #1e1e2e;
  color: #6c7086;
  font-weight: 500;
  text-align: left;
  padding: 6px 10px;
  border-bottom: 1px solid #313244;
  position: sticky;
  top: 0;
}

.table td {
  padding: 5px 10px;
  border-bottom: 1px solid #1e1e2e;
  cursor: pointer;
}

.table tbody tr:hover {
  background: #1e1e2e;
}

.table tbody tr.selected {
  background: #89b4fa;
  color: #1e1e2e;
}

.table tbody tr.selected .col-comment {
  color: #45475a;
}

.col-addr {
  font-family: 'SF Mono', 'Fira Code', monospace;
  width: 80px;
  color: #89b4fa;
}

.table tbody tr.selected .col-addr {
  color: #1e1e2e;
}

.col-name {
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.col-value {
  width: 100px;
}

.col-comment {
  color: #6c7086;
  font-size: 11px;
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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

.num-value {
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.edit-input {
  width: 70px;
  padding: 2px 6px;
  background: #1e1e2e;
  border: 1px solid #89b4fa;
  border-radius: 3px;
  color: #cdd6f4;
  font-family: monospace;
  font-size: 12px;
}

/* Context Menu */
.context-menu {
  position: fixed;
  background: #1e1e2e;
  border: 1px solid #45475a;
  border-radius: 6px;
  z-index: 999;
  min-width: 140px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
}

.context-menu-item {
  padding: 8px 14px;
  font-size: 13px;
  color: #cdd6f4;
  cursor: pointer;
  border-radius: 6px;
}

.context-menu-item:hover {
  background: #313244;
}

.context-menu-item.danger {
  color: #f38ba8;
}

.context-menu-item.danger:hover {
  background: #3d2a30;
}
</style>
