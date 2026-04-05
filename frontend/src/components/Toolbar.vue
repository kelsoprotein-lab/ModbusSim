<script setup lang="ts">
import { ref, inject, watch, onUnmounted, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { save, open } from '@tauri-apps/plugin-dialog'
import { dialogKey } from '../composables/useDialog'
import type { showAlert as ShowAlert, showConfirm as ShowConfirm, showPrompt as ShowPrompt } from '../composables/useDialog'

const selectedConnectionId = inject<Ref<string | null>>('selectedConnectionId')!
const selectedConnectionState = inject<Ref<string>>('selectedConnectionState')!
const selectedSlaveId = inject<Ref<number | null>>('selectedSlaveId')!
const refreshTree = inject<() => void>('refreshTree')!
const refreshRegisters = inject<() => void>('refreshRegisters')!
const { showAlert, showConfirm } = inject<{ showAlert: typeof ShowAlert; showConfirm: typeof ShowConfirm; showPrompt: typeof ShowPrompt }>(dialogKey)!

// --- Project File Management ---
const currentProjectPath = ref<string | null>(null)

async function openProject() {
  try {
    const path = await open({
      filters: [{ name: 'Modbus Project', extensions: ['modbusproj'] }],
    })
    if (!path) return
    await invoke('load_project_file', { path })
    currentProjectPath.value = path as string
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

async function saveProject() {
  if (!currentProjectPath.value) {
    return saveProjectAs()
  }
  try {
    await invoke('save_project_file', { path: currentProjectPath.value })
  } catch (e) {
    await showAlert(String(e))
  }
}

async function saveProjectAs() {
  try {
    const path = await save({
      filters: [{ name: 'Modbus Project', extensions: ['modbusproj'] }],
      defaultPath: 'untitled.modbusproj',
    })
    if (!path) return
    await invoke('save_project_file', { path })
    currentProjectPath.value = path
  } catch (e) {
    await showAlert(String(e))
  }
}

// --- New Connection Modal ---
const showNewConnModal = ref(false)
const newConnPort = ref('5020')
const newConnInitMode = ref('zero')

function openNewConnModal() {
  newConnPort.value = '5020'
  newConnInitMode.value = 'zero'
  showNewConnModal.value = true
}

async function submitNewConnection() {
  const port = Number(newConnPort.value)
  if (!port || port < 1 || port > 65535) {
    await showAlert('请输入有效的端口号 (1-65535)')
    return
  }
  showNewConnModal.value = false
  try {
    await invoke('create_slave_connection', {
      request: { port, init_mode: newConnInitMode.value }
    })
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

// --- New Slave Modal ---
const showNewSlaveModal = ref(false)
const newSlaveId = ref('2')
const newSlaveInitMode = ref('zero')

function openNewSlaveModal() {
  if (!selectedConnectionId.value) return
  newSlaveId.value = '2'
  newSlaveInitMode.value = 'zero'
  showNewSlaveModal.value = true
}

async function submitNewSlave() {
  const slaveId = Number(newSlaveId.value)
  if (!slaveId || slaveId < 1 || slaveId > 247) {
    await showAlert('请输入有效的从站 ID (1-247)')
    return
  }
  showNewSlaveModal.value = false
  try {
    await invoke('add_slave_device', {
      request: {
        connection_id: selectedConnectionId.value,
        slave_id: slaveId,
        name: `从站 ${slaveId}`,
        init_mode: newSlaveInitMode.value,
      }
    })
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

async function startConnection() {
  if (!selectedConnectionId.value) return
  try {
    await invoke('start_slave_connection', { id: selectedConnectionId.value })
    selectedConnectionState.value = 'Running'
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

async function stopConnection() {
  if (!selectedConnectionId.value) return
  try {
    await invoke('stop_slave_connection', { id: selectedConnectionId.value })
    selectedConnectionState.value = 'Stopped'
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

async function closeConnection() {
  if (!selectedConnectionId.value) return
  if (!(await showConfirm('确认删除此连接？'))) return
  try {
    await invoke('delete_slave_connection', { id: selectedConnectionId.value })
    selectedConnectionId.value = null
    selectedConnectionState.value = 'Stopped'
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

async function openTools() {
  await showAlert('工具面板（待实现）')
}

// --- Random Mutation ---
const mutationActive = ref(false)
const mutationRate = ref(1000)
const mutationTypes = ref<Record<string, boolean>>({
  coil: true,
  discrete_input: false,
  holding_register: true,
  input_register: false,
})
let mutationTimer: number | null = null

function toggleMutation() {
  if (mutationActive.value) {
    stopMutation()
  } else {
    startMutation()
  }
}

function startMutation() {
  if (!selectedConnectionId.value || selectedSlaveId.value === null) return
  mutationActive.value = true
  scheduleMutation()
}

function stopMutation() {
  mutationActive.value = false
  if (mutationTimer !== null) {
    clearTimeout(mutationTimer)
    mutationTimer = null
  }
}

function scheduleMutation() {
  if (!mutationActive.value) return
  mutationTimer = window.setTimeout(async () => {
    if (!mutationActive.value || !selectedConnectionId.value || selectedSlaveId.value === null) {
      stopMutation()
      return
    }
    const types = Object.entries(mutationTypes.value)
      .filter(([, v]) => v)
      .map(([k]) => k)
    if (types.length > 0) {
      try {
        await invoke('random_mutate_registers', {
          request: {
            connection_id: selectedConnectionId.value,
            slave_id: selectedSlaveId.value,
            register_types: types,
          }
        })
        refreshRegisters()
      } catch (e) { console.error('mutation failed:', e) }
    }
    scheduleMutation()
  }, mutationRate.value)
}

watch([selectedConnectionId, selectedSlaveId], () => {
  if (mutationActive.value) stopMutation()
})

onUnmounted(() => {
  if (mutationTimer !== null) clearTimeout(mutationTimer)
})
</script>

<template>
  <div class="toolbar">
    <div class="toolbar-group">
      <button class="toolbar-btn" @click="openProject" title="打开项目">
        <span class="toolbar-label">打开</span>
      </button>
      <button class="toolbar-btn" @click="saveProject" title="保存项目">
        <span class="toolbar-label">保存</span>
      </button>
      <button class="toolbar-btn" @click="saveProjectAs" title="另存为">
        <span class="toolbar-label">另存为</span>
      </button>
    </div>
    <div class="toolbar-divider"></div>
    <div class="toolbar-group">
      <button class="toolbar-btn" @click="openNewConnModal" title="新建连接">
        <span class="toolbar-icon">+</span>
        <span class="toolbar-label">新建连接</span>
      </button>
      <button
        class="toolbar-btn"
        @click="openNewSlaveModal"
        :disabled="!selectedConnectionId"
        title="新建从站"
      >
        <span class="toolbar-icon">+</span>
        <span class="toolbar-label">新建从站</span>
      </button>
    </div>
    <div class="toolbar-divider"></div>
    <div class="toolbar-group">
      <button
        class="toolbar-btn btn-start"
        @click="startConnection"
        :disabled="!selectedConnectionId || selectedConnectionState === 'Running'"
        title="启动连接"
      >
        <span class="toolbar-label">启动</span>
      </button>
      <button
        class="toolbar-btn btn-stop"
        @click="stopConnection"
        :disabled="!selectedConnectionId || selectedConnectionState === 'Stopped'"
        title="停止连接"
      >
        <span class="toolbar-label">停止</span>
      </button>
      <button
        class="toolbar-btn btn-close"
        @click="closeConnection"
        :disabled="!selectedConnectionId"
        title="关闭连接"
      >
        <span class="toolbar-label">关闭</span>
      </button>
    </div>
    <div class="toolbar-divider"></div>
    <div class="toolbar-group mutation-group">
      <button
        :class="['toolbar-btn', { 'btn-mutation-active': mutationActive }]"
        @click="toggleMutation"
        :disabled="!selectedConnectionId || selectedSlaveId === null"
        title="随机变位"
      >
        <span class="toolbar-label">{{ mutationActive ? '停止变位' : '随机变位' }}</span>
      </button>
      <input
        type="range"
        class="rate-slider"
        min="100"
        max="5000"
        step="100"
        v-model.number="mutationRate"
        title="变位间隔 (ms)"
      />
      <span class="rate-label">{{ mutationRate }}ms</span>
      <label v-for="(label, key) in { coil: '线圈', discrete_input: '离散输入', holding_register: '保持寄存器', input_register: '输入寄存器' }" :key="key" class="mutation-type-label">
        <input type="checkbox" v-model="mutationTypes[key]" />
        {{ label }}
      </label>
    </div>
    <div class="toolbar-divider"></div>
    <div class="toolbar-group">
      <button class="toolbar-btn" @click="openTools" title="工具">
        <span class="toolbar-label">工具</span>
      </button>
    </div>
    <div class="toolbar-title">ModbusSlave</div>
  </div>

  <!-- New Connection Modal -->
  <Teleport to="body">
    <div v-if="showNewConnModal" class="modal-overlay" @click.self="showNewConnModal = false">
      <div class="modal-box">
        <div class="modal-title">新建连接</div>
        <div class="modal-field">
          <label>端口号</label>
          <input
            v-model="newConnPort"
            type="number"
            min="1"
            max="65535"
            @keyup.enter="submitNewConnection"
          />
        </div>
        <div class="modal-field">
          <label>初始值</label>
          <div class="radio-group">
            <label class="radio-label">
              <input type="radio" v-model="newConnInitMode" value="zero" /> 全零
            </label>
            <label class="radio-label">
              <input type="radio" v-model="newConnInitMode" value="random" /> 随机
            </label>
          </div>
        </div>
        <div class="modal-actions">
          <button class="modal-btn cancel" @click="showNewConnModal = false">取消</button>
          <button class="modal-btn confirm" @click="submitNewConnection">确定</button>
        </div>
      </div>
    </div>
  </Teleport>

  <!-- New Slave Modal -->
  <Teleport to="body">
    <div v-if="showNewSlaveModal" class="modal-overlay" @click.self="showNewSlaveModal = false">
      <div class="modal-box">
        <div class="modal-title">新建从站</div>
        <div class="modal-field">
          <label>从站 ID</label>
          <input
            v-model="newSlaveId"
            type="number"
            min="1"
            max="247"
            @keyup.enter="submitNewSlave"
          />
        </div>
        <div class="modal-field">
          <label>初始值</label>
          <div class="radio-group">
            <label class="radio-label">
              <input type="radio" v-model="newSlaveInitMode" value="zero" /> 全零
            </label>
            <label class="radio-label">
              <input type="radio" v-model="newSlaveInitMode" value="random" /> 随机
            </label>
          </div>
        </div>
        <div class="modal-actions">
          <button class="modal-btn cancel" @click="showNewSlaveModal = false">取消</button>
          <button class="modal-btn confirm" @click="submitNewSlave">确定</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.toolbar {
  display: flex;
  align-items: center;
  height: 42px;
  padding: 0 8px;
  gap: 4px;
  user-select: none;
}

.toolbar-group {
  display: flex;
  gap: 2px;
}

.toolbar-divider {
  width: 1px;
  height: 24px;
  background: #313244;
  margin: 0 4px;
}

.toolbar-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 10px;
  border: none;
  background: transparent;
  color: #cdd6f4;
  cursor: pointer;
  border-radius: 4px;
  font-size: 12px;
  white-space: nowrap;
}

.toolbar-btn:hover:not(:disabled) {
  background: #313244;
}

.toolbar-btn:disabled {
  opacity: 0.4;
  cursor: default;
}

.toolbar-btn.btn-start:not(:disabled) {
  color: #a6e3a1;
}

.toolbar-btn.btn-stop:not(:disabled) {
  color: #fab387;
}

.toolbar-btn.btn-close:not(:disabled) {
  color: #f38ba8;
}

.toolbar-icon {
  font-weight: bold;
  font-size: 14px;
}

.toolbar-btn.btn-mutation-active {
  background: #a6e3a1;
  color: #1e1e2e;
  font-weight: 600;
}

.toolbar-btn.btn-mutation-active:hover {
  background: #94e2d5;
}

.mutation-group {
  align-items: center;
}

.rate-slider {
  width: 80px;
  height: 4px;
  accent-color: #89b4fa;
  cursor: pointer;
}

.rate-label {
  font-size: 10px;
  color: #6c7086;
  min-width: 42px;
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.mutation-type-label {
  display: flex;
  align-items: center;
  gap: 3px;
  font-size: 11px;
  color: #a6adc8;
  cursor: pointer;
  white-space: nowrap;
}

.mutation-type-label input[type="checkbox"] {
  accent-color: #89b4fa;
}

.toolbar-title {
  margin-left: auto;
  font-size: 12px;
  color: #6c7086;
  padding-right: 8px;
}

/* Modal styles */
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-box {
  background: #1e1e2e;
  border: 1px solid #45475a;
  border-radius: 8px;
  padding: 20px;
  min-width: 300px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
}

.modal-title {
  font-size: 14px;
  font-weight: 600;
  color: #cdd6f4;
  margin-bottom: 16px;
}

.modal-field {
  margin-bottom: 14px;
}

.modal-field label {
  display: block;
  font-size: 12px;
  color: #a6adc8;
  margin-bottom: 6px;
}

.modal-field input[type="number"] {
  width: 100%;
  padding: 6px 10px;
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  font-size: 13px;
  outline: none;
}

.modal-field input[type="number"]:focus {
  border-color: #89b4fa;
}

.radio-group {
  display: flex;
  gap: 16px;
}

.radio-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: #cdd6f4;
  cursor: pointer;
}

.radio-label input[type="radio"] {
  accent-color: #89b4fa;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 18px;
}

.modal-btn {
  padding: 6px 16px;
  border: none;
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
}

.modal-btn.cancel {
  background: #313244;
  color: #a6adc8;
}

.modal-btn.cancel:hover {
  background: #45475a;
}

.modal-btn.confirm {
  background: #89b4fa;
  color: #1e1e2e;
  font-weight: 600;
}

.modal-btn.confirm:hover {
  background: #74c7ec;
}
</style>
