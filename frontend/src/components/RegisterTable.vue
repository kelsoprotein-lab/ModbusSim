<script setup lang="ts">
import { ref, inject, watch, computed, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { dialogKey } from '../composables/useDialog'
import type { showAlert as ShowAlert } from '../composables/useDialog'

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
  (e: 'register-select', reg: { address: number; register_type: string; value: number } | null): void
}>()

const selectedConnectionId = inject<Ref<string | null>>('selectedConnectionId')!
const selectedSlaveId = inject<Ref<number | null>>('selectedSlaveId')!
const selectedRegisterType = inject<Ref<string | null>>('selectedRegisterType')!

const registers = ref<Register[]>([])
const selectedRow = ref<Register | null>(null)
const registerValues = ref<Record<string, number>>({})
const editingCell = ref<{ address: number; register_type: string } | null>(null)
const editValue = ref('')
const isLoading = ref(false)
const error = ref<string | null>(null)
const contextMenu = ref({ show: false, x: 0, y: 0, reg: null as Register | null })

// Filter registers by selected type
const filteredRegisters = computed(() => {
  if (!selectedRegisterType.value) return registers.value
  return registers.value.filter(r => r.register_type === selectedRegisterType.value)
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

    // Load values for non-bool types
    const values: Record<string, number> = {}
    for (const reg of defs) {
      if (reg.register_type !== 'coil' && reg.register_type !== 'discrete_input') {
        const result = await invoke<{ address: number; value: number }>('read_register', {
          connectionId: selectedConnectionId.value,
          slaveId: selectedSlaveId.value,
          registerType: reg.register_type,
          address: reg.address,
        })
        values[`${reg.register_type}-${reg.address}`] = result.value
      } else {
        const result = await invoke<{ address: number; value: number }>('read_register', {
          connectionId: selectedConnectionId.value,
          slaveId: selectedSlaveId.value,
          registerType: reg.register_type,
          address: reg.address,
        })
        values[`${reg.register_type}-${reg.address}`] = result.value
      }
    }
    registerValues.value = values
  } catch (e) {
    error.value = String(e)
  }
  isLoading.value = false
}

watch([selectedConnectionId, selectedSlaveId, selectedRegisterType], () => {
  selectedRow.value = null
  emit('register-select', null)
  loadRegisters()
})

function getValue(reg: Register): number {
  return registerValues.value[`${reg.register_type}-${reg.address}`] ?? 0
}

function selectRow(reg: Register) {
  selectedRow.value = reg
  const val = getValue(reg)
  emit('register-select', { address: reg.address, register_type: reg.register_type, value: val })
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

    // Update selected row value if it was the one edited
    if (selectedRow.value && selectedRow.value.address === address && selectedRow.value.register_type === register_type) {
      emit('register-select', { address, register_type, value: registerValues.value[`${register_type}-${address}`] })
    }
  } catch (e) {
    await showAlert(String(e))
  }
  editingCell.value = null
}

function cancelEdit() {
  editingCell.value = null
}

function handleKeydown(e: KeyboardEvent) {
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
    if (selectedRow.value && selectedRow.value.address === reg.address) {
      selectedRow.value = null
      emit('register-select', null)
    }
    await loadRegisters()
  } catch (e) {
    await showAlert(String(e))
  }
}

function formatAddress(reg: Register): string {
  return '0x' + reg.address.toString(16).toUpperCase().padStart(4, '0')
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
      <span class="table-count">{{ filteredRegisters.length }} 个寄存器</span>
    </div>

    <div v-if="isLoading" class="table-loading">加载中...</div>
    <div v-else-if="!selectedConnectionId || selectedSlaveId === null" class="table-empty">
      请在左侧树形导航中选择一个从站
    </div>
    <div v-else-if="filteredRegisters.length === 0" class="table-empty">
      暂无寄存器
    </div>

    <template v-else>
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
            :class="{ selected: selectedRow === reg }"
            @click="selectRow(reg)"
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
                  @keydown="handleKeydown"
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
    </template>

    <!-- Context Menu -->
    <div
      v-if="contextMenu.show"
      class="context-menu"
      :style="{ top: contextMenu.y + 'px', left: contextMenu.x + 'px' }"
      @click.stop
    >
      <div class="context-menu-item danger" @click="deleteRegister">删除寄存器</div>
    </div>
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
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid #313244;
  flex-shrink: 0;
}

.table-title {
  font-size: 12px;
  font-weight: 600;
  color: #cdd6f4;
}

.table-count {
  font-size: 11px;
  color: #6c7086;
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

.table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
  flex: 1;
  overflow-y: auto;
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
