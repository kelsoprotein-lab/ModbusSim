<script setup lang="ts">
import { ref, inject, watch, computed, provide, onUnmounted, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { dialogKey } from '../composables/useDialog'
import type { showAlert as ShowAlert } from '../composables/useDialog'
import { swapBytes16, toFloat32, float32ToU16Pair, type ByteOrder } from 'shared-frontend'
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
// scrollContainer removed — replaced by scrollContainerRef for virtual scrolling
const addrMode = ref<'hex' | 'dec'>('hex')
provide('addrMode', addrMode)
const showAddModal = ref(false)
const showBatchModal = ref(false)

type ValueFormat = 'auto' | 'unsigned' | 'signed' | 'hex' | 'binary' | 'float32_abcd' | 'float32_cdab' | 'float32_badc' | 'float32_dcba'
const valueFormat = ref<ValueFormat>('auto')

const formatOptions: { value: ValueFormat; label: string }[] = [
  { value: 'auto', label: 'Auto' },
  { value: 'unsigned', label: 'Unsigned' },
  { value: 'signed', label: 'Signed' },
  { value: 'hex', label: 'Hex' },
  { value: 'binary', label: 'Binary' },
  { value: 'float32_abcd', label: 'Float AB CD' },
  { value: 'float32_cdab', label: 'Float CD AB' },
  { value: 'float32_badc', label: 'Float BA DC' },
  { value: 'float32_dcba', label: 'Float DC BA' },
]

const isFloatFormat = computed(() => valueFormat.value.startsWith('float32_'))

function formatValue(raw: number): string {
  const v = raw & 0xFFFF
  switch (valueFormat.value) {
    case 'signed': return (v >= 0x8000 ? v - 0x10000 : v).toString()
    case 'hex': return '0x' + v.toString(16).toUpperCase().padStart(4, '0')
    case 'binary': {
      const b = v.toString(2).padStart(16, '0')
      return `${b.slice(0, 4)} ${b.slice(4, 8)} ${b.slice(8, 12)} ${b.slice(12, 16)}`
    }
    default: return v.toString()
  }
}

function is32BitType(dataType: string): boolean {
  return dataType === 'uint32' || dataType === 'int32' || dataType === 'float32'
}

const companionKeys = computed(() => {
  const set = new Set<string>()
  for (const reg of registers.value) {
    if (is32BitType(reg.data_type)) {
      set.add(`${reg.register_type}-${reg.address + 1}`)
    }
  }
  return set
})

function isCompanionRegister(reg: Register): boolean {
  if (is32BitType(reg.data_type)) return false
  return companionKeys.value.has(`${reg.register_type}-${reg.address}`)
}

function applyEndianDecode(reg0: number, reg1: number, endian: string): [number, number, number, number] {
  const r0h = (reg0 >> 8) & 0xFF, r0l = reg0 & 0xFF
  const r1h = (reg1 >> 8) & 0xFF, r1l = reg1 & 0xFF
  switch (endian) {
    case 'big':        return [r0h, r0l, r1h, r1l]
    case 'little':     return [r1h, r1l, r0h, r0l]
    case 'mid_big':    return [r0l, r0h, r1l, r1h]
    case 'mid_little': return [r1l, r1h, r0l, r0h]
    default:           return [r0h, r0l, r1h, r1l]
  }
}

function formatTypedValue(reg: Register): string {
  const hi = getValue(reg)
  switch (reg.data_type) {
    case 'bool': return hi !== 0 ? 'ON' : 'OFF'
    case 'uint16': return (hi & 0xFFFF).toString()
    case 'int16': { const v = hi & 0xFFFF; return (v >= 0x8000 ? v - 0x10000 : v).toString() }
    case 'uint32': case 'int32': case 'float32': {
      const lo = registerValues.value[`${reg.register_type}-${reg.address + 1}`] ?? 0
      const bytes = applyEndianDecode(hi & 0xFFFF, lo & 0xFFFF, reg.endian)
      const canonHi = (bytes[0] << 8) | bytes[1]
      const canonLo = (bytes[2] << 8) | bytes[3]
      if (reg.data_type === 'float32') return toFloat32(canonHi, canonLo)
      const buf = new ArrayBuffer(4)
      const view = new DataView(buf)
      view.setUint16(0, canonHi)
      view.setUint16(2, canonLo)
      if (reg.data_type === 'uint32') return view.getUint32(0).toString()
      return view.getInt32(0).toString()
    }
    default: return (hi & 0xFFFF).toString()
  }
}

const endianToByteOrder: Record<string, ByteOrder> = { big: 'ABCD', little: 'CDAB', mid_big: 'BADC', mid_little: 'DCBA' }

function encodeTypedValue(value: number, dataType: string, endian: string): [number, number] {
  if (dataType === 'float32') return float32ToU16Pair(value, endianToByteOrder[endian] || 'ABCD')
  const buf = new ArrayBuffer(4)
  const view = new DataView(buf)
  if (dataType === 'int32') view.setInt32(0, value)
  else view.setUint32(0, value >>> 0)
  const w0 = view.getUint16(0), w1 = view.getUint16(2)
  const order = endianToByteOrder[endian] || 'ABCD'
  switch (order) {
    case 'ABCD': return [w0, w1]
    case 'CDAB': return [w1, w0]
    case 'BADC': return [swapBytes16(w0), swapBytes16(w1)]
    case 'DCBA': return [swapBytes16(w1), swapBytes16(w0)]
  }
}

// Float 格式：地址连续的寄存器配对，第二个为伴随
const floatCompanionIndices = computed(() => {
  if (!isFloatFormat.value) return new Set<number>()
  const set = new Set<number>()
  const list = filteredRegisters.value
  let i = 0
  while (i < list.length - 1) {
    if (list[i + 1].address === list[i].address + 1 && list[i + 1].register_type === list[i].register_type) {
      set.add(i + 1)
      i += 2
    } else {
      i += 1
    }
  }
  return set
})

function formatFloatPair(reg: Register, nextReg: Register | undefined): string {
  const hi = getValue(reg) & 0xFFFF
  const lo = nextReg ? getValue(nextReg) & 0xFFFF : 0
  switch (valueFormat.value) {
    case 'float32_abcd': return toFloat32(hi, lo)
    case 'float32_cdab': return toFloat32(lo, hi)
    case 'float32_badc': return toFloat32(swapBytes16(hi), swapBytes16(lo))
    case 'float32_dcba': return toFloat32(swapBytes16(lo), swapBytes16(hi))
    default: return toFloat32(hi, lo)
  }
}

function isDisplayCompanion(reg: Register, index: number): boolean {
  if (valueFormat.value === 'auto') return isCompanionRegister(reg)
  if (isFloatFormat.value) return floatCompanionIndices.value.has(index)
  return false
}

function getDisplayValue(reg: Register, index: number): string {
  if (valueFormat.value === 'auto') return formatTypedValue(reg)
  if (isFloatFormat.value) {
    const list = filteredRegisters.value
    const next = index + 1 < list.length ? list[index + 1] : undefined
    const nextReg = next && next.address === reg.address + 1 && next.register_type === reg.register_type ? next : undefined
    return formatFloatPair(reg, nextReg)
  }
  return formatValue(getValue(reg))
}

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

// Virtual scrolling
const scrollContainerRef = ref<HTMLElement | null>(null)
const ROW_HEIGHT = 32

const rowVirtualizer = useVirtualizer(computed(() => ({
  count: filteredRegisters.value.length,
  getScrollElement: () => scrollContainerRef.value,
  estimateSize: () => ROW_HEIGHT,
  overscan: 5,
})))

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

watch([selectedConnectionId, selectedSlaveId, selectedRegisterType], async () => {
  clearSelection()
  stopAutoRefresh()
  await loadRegisters()
  if (registers.value.length > 0) startAutoRefresh()
})

watch(registerRefreshKey, () => refreshValues())

// Auto-refresh register values every 2s to pick up external writes (e.g. from Master)
let refreshTimer: number | null = null

async function refreshValues() {
  if (!selectedConnectionId.value || selectedSlaveId.value === null) return
  if (registers.value.length === 0) return
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
}

function startAutoRefresh() {
  stopAutoRefresh()
  refreshTimer = window.setInterval(refreshValues, 2000)
}

function stopAutoRefresh() {
  if (refreshTimer) { clearInterval(refreshTimer); refreshTimer = null }
}

onUnmounted(() => stopAutoRefresh())

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

      // Scroll into view via virtualizer
      rowVirtualizer.value.scrollToIndex(nextIdx, { align: 'auto' })
    }
  }
}

function startEdit(reg: Register) {
  const idx = filteredRegisters.value.indexOf(reg)
  if (isDisplayCompanion(reg, idx)) return
  editingCell.value = { address: reg.address, register_type: reg.register_type }
  if (valueFormat.value === 'auto' || isFloatFormat.value) {
    editValue.value = getDisplayValue(reg, idx)
  } else {
    editValue.value = String(getValue(reg))
  }
}

async function commitEdit() {
  if (!editingCell.value || !selectedConnectionId.value || selectedSlaveId.value === null) return
  const { address, register_type } = editingCell.value
  const reg = registers.value.find(r => r.address === address && r.register_type === register_type)

  const needsFloat32Write = isFloatFormat.value || (valueFormat.value === 'auto' && reg && is32BitType(reg.data_type))

  if (needsFloat32Write) {
    const inputVal = parseFloat(editValue.value)
    if (isNaN(inputVal)) { editingCell.value = null; return }
    let hi: number, lo: number
    if (isFloatFormat.value) {
      const order = (valueFormat.value.replace('float32_', '').toUpperCase() as ByteOrder) || 'ABCD'
      ;[hi, lo] = float32ToU16Pair(inputVal, order)
    } else {
      ;[hi, lo] = encodeTypedValue(inputVal, reg!.data_type, reg!.endian)
    }
    try {
      await invoke('write_register', {
        request: { connection_id: selectedConnectionId.value, slave_id: selectedSlaveId.value, register_type, address, value: hi }
      })
      await invoke('write_register', {
        request: { connection_id: selectedConnectionId.value, slave_id: selectedSlaveId.value, register_type, address: address + 1, value: lo }
      })
      registerValues.value[`${register_type}-${address}`] = hi
      registerValues.value[`${register_type}-${address + 1}`] = lo
      emitSelection()
    } catch (e) {
      await showAlert(String(e))
    }
  } else if (valueFormat.value === 'auto' && reg && reg.data_type === 'int16') {
    const inputVal = Number(editValue.value)
    if (isNaN(inputVal)) { editingCell.value = null; return }
    const value = inputVal < 0 ? (inputVal + 0x10000) & 0xFFFF : inputVal & 0xFFFF
    try {
      await invoke('write_register', {
        request: { connection_id: selectedConnectionId.value, slave_id: selectedSlaveId.value, register_type, address, value }
      })
      registerValues.value[`${register_type}-${address}`] = value
      emitSelection()
    } catch (e) {
      await showAlert(String(e))
    }
  } else {
    const value = Number(editValue.value)
    if (isNaN(value)) { editingCell.value = null; return }
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
      <select v-model="valueFormat" class="format-select" title="值显示格式">
        <option v-for="opt in formatOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
      </select>
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
      ref="scrollContainerRef"
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
      </table>
      <div :style="{ height: `${rowVirtualizer.getTotalSize()}px`, position: 'relative' }">
        <div
          v-for="virtualRow in rowVirtualizer.getVirtualItems()"
          :key="filteredRegisters[virtualRow.index]?.address ?? virtualRow.index"
          class="virtual-row"
          :class="{ selected: isSelected(filteredRegisters[virtualRow.index]) }"
          :style="{
            position: 'absolute',
            top: 0,
            left: 0,
            width: '100%',
            height: `${ROW_HEIGHT}px`,
            transform: `translateY(${virtualRow.start}px)`,
          }"
          @click="selectRow($event, filteredRegisters[virtualRow.index])"
          @contextmenu.prevent="showContextMenu($event, filteredRegisters[virtualRow.index])"
        >
          <span class="vcol col-addr">{{ formatAddress(filteredRegisters[virtualRow.index]) }}</span>
          <span class="vcol col-name">{{ filteredRegisters[virtualRow.index].name || '-' }}</span>
          <span :class="['vcol', 'col-value', { wide: valueFormat === 'auto' }]" @dblclick.stop="startEdit(filteredRegisters[virtualRow.index])">
            <template v-if="editingCell?.address === filteredRegisters[virtualRow.index].address && editingCell?.register_type === filteredRegisters[virtualRow.index].register_type">
              <input
                v-model="editValue"
                class="edit-input"
                :type="valueFormat === 'auto' || isFloatFormat ? 'text' : 'number'"
                autofocus
                @blur="commitEdit"
                @keydown="handleEditKeydown"
                @click.stop
              />
            </template>
            <template v-else>
              <span
                v-if="filteredRegisters[virtualRow.index].register_type === 'coil' || filteredRegisters[virtualRow.index].register_type === 'discrete_input'"
                :class="['bool-value', getValue(filteredRegisters[virtualRow.index]) ? 'on' : 'off']"
              >
                {{ getValue(filteredRegisters[virtualRow.index]) ? 'ON' : 'OFF' }}
              </span>
              <span v-else-if="isDisplayCompanion(filteredRegisters[virtualRow.index], virtualRow.index)" class="companion-value">&#x22EF;</span>
              <span v-else-if="valueFormat === 'auto' || isFloatFormat" class="num-value" :title="valueFormat === 'auto' ? `${filteredRegisters[virtualRow.index].data_type} (${filteredRegisters[virtualRow.index].endian})` : ''">{{ getDisplayValue(filteredRegisters[virtualRow.index], virtualRow.index) }}</span>
              <span v-else class="num-value">{{ formatValue(getValue(filteredRegisters[virtualRow.index])) }}</span>
            </template>
          </span>
          <span class="vcol col-comment">{{ filteredRegisters[virtualRow.index].comment || '-' }}</span>
        </div>
      </div>
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

.format-select {
  padding: 2px 6px;
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  font-size: 11px;
  cursor: pointer;
}

.format-select:focus {
  outline: none;
  border-color: #89b4fa;
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
  contain: strict;
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

.companion-value {
  color: #45475a;
  font-size: 11px;
  font-style: italic;
}

.edit-input {
  width: 90px;
  padding: 2px 6px;
  background: #1e1e2e;
  border: 1px solid #89b4fa;
  border-radius: 3px;
  color: #cdd6f4;
  font-family: monospace;
  font-size: 12px;
}

/* Virtual rows */
.virtual-row {
  display: flex;
  align-items: center;
  cursor: pointer;
  font-size: 12px;
  border-bottom: 1px solid #1e1e2e;
}

.virtual-row:hover {
  background: #1e1e2e;
}

.virtual-row.selected {
  background: #89b4fa;
  color: #1e1e2e;
}

.virtual-row.selected .col-addr {
  color: #1e1e2e;
}

.virtual-row.selected .col-comment {
  color: #45475a;
}

.vcol {
  padding: 5px 10px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.vcol.col-addr {
  width: 80px;
  min-width: 80px;
}

.vcol.col-name {
  width: 120px;
  min-width: 120px;
}

.vcol.col-value {
  width: 100px;
  min-width: 100px;
}

.vcol.col-value.wide {
  width: 140px;
  min-width: 140px;
}

.vcol.col-comment {
  flex: 1;
  min-width: 0;
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
