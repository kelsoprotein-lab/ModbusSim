<script setup lang="ts">
import { ref, inject, watch, computed, provide, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { float32ToU16Pair, useI18n, useFcLabel, formatAddress, showAlert, type ByteOrder } from 'shared-frontend'
import RegisterModal from './RegisterModal.vue'
import BatchAddModal from './BatchAddModal.vue'
import { useRegisterValues } from '../composables/useRegisterValues'
import {
  formatU16, formatTypedValue, formatFloatPair, encodeTypedValue,
  is32BitType, isFloatFormat as isFloatFmt,
  type RegisterDef as Register, type ValueFormat,
} from '../composables/useRegisterFormat'

const { t } = useI18n()
const { registerTypeLabel } = useFcLabel()

const emit = defineEmits<{
  (e: 'register-select', regs: { address: number; register_type: string; value: number }[]): void
}>()

const selectedConnectionId = inject<Ref<string | null>>('selectedConnectionId')!
const selectedSlaveId = inject<Ref<number | null>>('selectedSlaveId')!
const selectedRegisterType = inject<Ref<string | null>>('selectedRegisterType')!
const registerRefreshKey = inject<Ref<number>>('registerRefreshKey')!

const {
  registers, registerValues, isLoading, error, changedKeys,
  loadRegisters, refreshValues, clearChangeTimers, getValue: getValueByKey,
} = useRegisterValues(selectedConnectionId, selectedSlaveId)

const selectedRows = ref<Register[]>([])
const lastClickedIndex = ref<number>(-1)
const editingCell = ref<{ address: number; register_type: string } | null>(null)
const editValue = ref('')
const searchQuery = ref('')
const contextMenu = ref({ show: false, x: 0, y: 0, reg: null as Register | null })
const addrMode = ref<'hex' | 'dec'>('hex')
provide('addrMode', addrMode)
const showAddModal = ref(false)
const showBatchModal = ref(false)

const valueFormat = ref<ValueFormat>('auto')

const formatOptions = computed<{ value: ValueFormat; label: string }[]>(() => [
  { value: 'auto', label: t('formats.auto') },
  { value: 'unsigned', label: t('formats.unsigned') },
  { value: 'signed', label: t('formats.signed') },
  { value: 'hex', label: t('formats.hex') },
  { value: 'binary', label: t('formats.binary') },
  { value: 'float32_abcd', label: t('formats.floatABCD') },
  { value: 'float32_cdab', label: t('formats.floatCDAB') },
  { value: 'float32_badc', label: t('formats.floatBADC') },
  { value: 'float32_dcba', label: t('formats.floatDCBA') },
])

const isFloatFormat = computed(() => isFloatFmt(valueFormat.value))

function getValue(reg: Register): number {
  return getValueByKey(reg.register_type, reg.address)
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

function isDisplayCompanion(reg: Register, index: number): boolean {
  if (valueFormat.value === 'auto') return isCompanionRegister(reg)
  if (isFloatFormat.value) return floatCompanionIndices.value.has(index)
  return false
}

function getDisplayValue(reg: Register, index: number): string {
  if (valueFormat.value === 'auto') {
    const lo = registerValues.value[`${reg.register_type}-${reg.address + 1}`] ?? 0
    return formatTypedValue(reg, getValue(reg), lo)
  }
  if (isFloatFormat.value) {
    const list = filteredRegisters.value
    const nextCandidate = index + 1 < list.length ? list[index + 1] : undefined
    const nextReg = nextCandidate && nextCandidate.address === reg.address + 1 && nextCandidate.register_type === reg.register_type
      ? nextCandidate
      : undefined
    return formatFloatPair(valueFormat.value, getValue(reg), nextReg ? getValue(nextReg) : 0)
  }
  return formatU16(getValue(reg), valueFormat.value)
}

function onRegisterSaved() {
  registerRefreshKey.value++
}

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

const scrollContainerRef = ref<HTMLElement | null>(null)
const ROW_HEIGHT = 32

const rowVirtualizer = useVirtualizer(computed(() => ({
  count: filteredRegisters.value.length,
  getScrollElement: () => scrollContainerRef.value,
  estimateSize: () => ROW_HEIGHT,
  overscan: 5,
})))

watch(searchQuery, () => {
  clearSelection()
})

watch([selectedConnectionId, selectedSlaveId, selectedRegisterType], async () => {
  clearSelection()
  clearChangeTimers()
  await loadRegisters()
})

watch(registerRefreshKey, async () => {
  await refreshValues()
  emitSelection()
})

function clearSelection() {
  selectedRows.value = []
  lastClickedIndex.value = -1
  emitSelection()
}

function isSelected(reg: Register): boolean {
  return selectedRows.value.some(r => r.address === reg.address && r.register_type === reg.register_type)
}

function selectRow(e: MouseEvent, reg: Register) {
  const list = filteredRegisters.value
  const idx = list.indexOf(reg)
  const isCtrl = e.ctrlKey || e.metaKey

  if (e.shiftKey && lastClickedIndex.value >= 0) {
    const start = Math.min(lastClickedIndex.value, idx)
    const end = Math.max(lastClickedIndex.value, idx)
    selectedRows.value = list.slice(start, end + 1)
  } else if (isCtrl) {
    if (isSelected(reg)) {
      selectedRows.value = selectedRows.value.filter(r => !(r.address === reg.address && r.register_type === reg.register_type))
    } else {
      selectedRows.value = [...selectedRows.value, reg]
    }
    lastClickedIndex.value = idx
  } else {
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
  if (editingCell.value) return

  const list = filteredRegisters.value
  if (list.length === 0) return

  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault()
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

function isBitType(rt: string): boolean {
  return rt === 'coil' || rt === 'discrete_input'
}

async function applyWrites(register_type: string, writes: Array<[number, number]>): Promise<boolean> {
  try {
    for (const [addr, value] of writes) {
      await invoke('write_register', {
        request: {
          connection_id: selectedConnectionId.value,
          slave_id: selectedSlaveId.value,
          register_type,
          address: addr,
          value,
        }
      })
      registerValues.value[`${register_type}-${addr}`] = value
    }
    emitSelection()
    return true
  } catch (e) {
    await showAlert(String(e))
    return false
  }
}

async function commitEdit() {
  if (!editingCell.value || !selectedConnectionId.value || selectedSlaveId.value === null) return
  const { address, register_type } = editingCell.value
  const reg = registers.value.find(r => r.address === address && r.register_type === register_type)
  editingCell.value = null

  const needsFloat32Write = isFloatFormat.value || (valueFormat.value === 'auto' && reg && is32BitType(reg.data_type))

  if (needsFloat32Write) {
    const inputVal = parseFloat(editValue.value)
    if (isNaN(inputVal)) return
    let hi: number, lo: number
    if (isFloatFormat.value) {
      const order = (valueFormat.value.replace('float32_', '').toUpperCase() as ByteOrder) || 'ABCD'
      ;[hi, lo] = float32ToU16Pair(inputVal, order)
    } else {
      ;[hi, lo] = encodeTypedValue(inputVal, reg!.data_type, reg!.endian)
    }
    await applyWrites(register_type, [[address, hi], [address + 1, lo]])
    return
  }

  if (valueFormat.value === 'auto' && reg && reg.data_type === 'int16') {
    const inputVal = Number(editValue.value)
    if (isNaN(inputVal)) return
    const value = inputVal < 0 ? (inputVal + 0x10000) & 0xFFFF : inputVal & 0xFFFF
    await applyWrites(register_type, [[address, value]])
    return
  }

  const inputVal = Number(editValue.value)
  if (isNaN(inputVal)) return
  const value = isBitType(register_type) ? (inputVal !== 0 ? 1 : 0) : inputVal
  await applyWrites(register_type, [[address, value]])
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

function fmtAddress(reg: Register): string {
  return formatAddress(reg.address, addrMode.value)
}

function toggleAddrMode() {
  addrMode.value = addrMode.value === 'hex' ? 'dec' : 'hex'
}
</script>

<template>
  <div class="register-table" @click="closeContextMenu">
    <div class="table-header-bar">
      <span class="table-title">
        {{ selectedRegisterType ? registerTypeLabel(selectedRegisterType) : t('table.allRegisters') }}
      </span>
      <input
        v-model="searchQuery"
        class="search-input"
        type="text"
        :placeholder="t('registerTable.searchPlaceholder')"
      />
      <button class="addr-mode-btn" @click="toggleAddrMode" :title="addrMode === 'hex' ? t('registerTable.switchToDecimal') : t('registerTable.switchToHex')">
        {{ addrMode === 'hex' ? 'HEX' : 'DEC' }}
      </button>
      <select v-model="valueFormat" class="format-select" :title="t('registerEdit.advanced')">
        <option v-for="opt in formatOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
      </select>
      <span v-if="error" class="table-error" :title="error">!</span>
      <button
        class="add-reg-btn"
        :disabled="!selectedConnectionId || selectedSlaveId === null"
        @click="showAddModal = true"
        :title="t('registerTable.addRegister')"
      >+</button>
      <button
        class="add-reg-btn batch"
        :disabled="!selectedConnectionId || selectedSlaveId === null"
        @click="showBatchModal = true"
        :title="t('registerTable.batchAddTitle')"
      >{{ t('registerTable.batchAdd') }}</button>
      <span class="table-count">{{ t('table.registerCount', { count: filteredRegisters.length }) }}</span>
    </div>

    <div v-if="isLoading" class="table-loading">{{ t('common.loading') }}</div>
    <div v-else-if="!selectedConnectionId || selectedSlaveId === null" class="table-empty">
      {{ t('registerTable.selectSlave') }}
    </div>
    <div v-else-if="filteredRegisters.length === 0" class="table-empty">
      {{ t('registerTable.noRegisters') }}
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
            <th>{{ t('table.address') }}</th>
            <th>{{ t('dialog.simpleName') }}</th>
            <th>{{ t('dialog.simpleValue') }}</th>
            <th>{{ t('table.comment') }}</th>
          </tr>
        </thead>
      </table>
      <div :style="{ height: `${rowVirtualizer.getTotalSize()}px`, position: 'relative' }">
        <div
          v-for="virtualRow in rowVirtualizer.getVirtualItems()"
          :key="filteredRegisters[virtualRow.index]?.address ?? virtualRow.index"
          class="virtual-row"
          :class="{
            selected: isSelected(filteredRegisters[virtualRow.index]),
            'value-changed': changedKeys.has(`${filteredRegisters[virtualRow.index].register_type}-${filteredRegisters[virtualRow.index].address}`)
          }"
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
          <span class="vcol col-addr">{{ fmtAddress(filteredRegisters[virtualRow.index]) }}</span>
          <span class="vcol col-name">{{ filteredRegisters[virtualRow.index].name || '-' }}</span>
          <span :class="['vcol', 'col-value', { wide: valueFormat === 'auto', 'value-highlight': changedKeys.has(`${filteredRegisters[virtualRow.index].register_type}-${filteredRegisters[virtualRow.index].address}`) }]" @dblclick.stop="startEdit(filteredRegisters[virtualRow.index])">
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
              <span v-else class="num-value">{{ formatU16(getValue(filteredRegisters[virtualRow.index]), valueFormat) }}</span>
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
      <div class="context-menu-item danger" @click="deleteRegister">{{ t('registerEdit.deleteRegister') }}</div>
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

.table-error {
  color: #f38ba8;
  font-weight: 700;
  font-size: 14px;
  cursor: help;
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

.virtual-row.value-changed {
  background: rgba(250, 179, 135, 0.18);
  transition: background 0.6s ease-out;
}

.virtual-row.value-changed.selected {
  background: #89b4fa;
}

.col-value.value-highlight {
  color: #fab387;
  font-weight: 700;
  transition: color 0.6s ease-out;
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
