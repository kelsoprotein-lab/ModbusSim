<script setup lang="ts">
import { inject, ref, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { dialogKey } from '../composables/useDialog'
import type { showAlert as ShowAlert } from '../composables/useDialog'
import ScanDialog from './ScanDialog.vue'

const { showAlert } = inject<{ showAlert: typeof ShowAlert }>(dialogKey)!
const selectedConnectionId = inject<Ref<string | null>>('selectedConnectionId')!
const selectedConnectionState = inject<Ref<string>>('selectedConnectionState')!
const refreshTree = inject<() => void>('refreshTree')!

// New Connection modal
const showNewConn = ref(false)
const newConnForm = ref({
  target_address: '127.0.0.1',
  port: 502,
  slave_id: 1,
  timeout_ms: 3000,
})

// Scan Group modal
const showNewScanGroup = ref(false)
const scanGroupForm = ref({
  name: '',
  function: 'read_holding_registers',
  start_address: 0,
  quantity: 10,
  interval_ms: 1000,
})

// Write modal
const showWriteModal = ref(false)
const showScanDialog = ref(false)
const writeForm = ref({
  function: 'write_single_register',
  address: 0,
  value: '',
})

async function createConnection() {
  try {
    await invoke('create_master_connection', {
      request: {
        target_address: newConnForm.value.target_address,
        port: newConnForm.value.port,
        slave_id: newConnForm.value.slave_id,
        timeout_ms: newConnForm.value.timeout_ms,
      }
    })
    showNewConn.value = false
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

async function connectMaster() {
  if (!selectedConnectionId.value) return
  try {
    await invoke('connect_master', { connectionId: selectedConnectionId.value })
    selectedConnectionState.value = 'Connected'
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

async function disconnectMaster() {
  if (!selectedConnectionId.value) return
  try {
    await invoke('disconnect_master', { connectionId: selectedConnectionId.value })
    selectedConnectionState.value = 'Disconnected'
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

async function deleteMaster() {
  if (!selectedConnectionId.value) return
  try {
    await invoke('delete_master_connection', { connectionId: selectedConnectionId.value })
    selectedConnectionId.value = null
    selectedConnectionState.value = 'Disconnected'
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

async function addScanGroup() {
  if (!selectedConnectionId.value) return
  try {
    await invoke('add_scan_group', {
      connectionId: selectedConnectionId.value,
      request: {
        name: scanGroupForm.value.name || `SG-${Date.now() % 10000}`,
        function: scanGroupForm.value.function,
        start_address: scanGroupForm.value.start_address,
        quantity: scanGroupForm.value.quantity,
        interval_ms: scanGroupForm.value.interval_ms,
      }
    })
    showNewScanGroup.value = false
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

async function startAllPolling() {
  if (!selectedConnectionId.value) return
  try {
    await invoke('start_all_polling', { connectionId: selectedConnectionId.value })
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

async function stopAllPolling() {
  if (!selectedConnectionId.value) return
  try {
    await invoke('stop_all_polling', { connectionId: selectedConnectionId.value })
    refreshTree()
  } catch (e) {
    await showAlert(String(e))
  }
}

async function submitWrite() {
  if (!selectedConnectionId.value) return
  try {
    const fc = writeForm.value.function
    if (fc === 'write_single_register') {
      await invoke('write_single_register', {
        connectionId: selectedConnectionId.value,
        request: { address: writeForm.value.address, value: parseInt(writeForm.value.value) }
      })
    } else if (fc === 'write_single_coil') {
      await invoke('write_single_coil', {
        connectionId: selectedConnectionId.value,
        request: { address: writeForm.value.address, value: writeForm.value.value === '1' || writeForm.value.value.toLowerCase() === 'true' }
      })
    } else if (fc === 'write_multiple_registers') {
      const values = writeForm.value.value.split(',').map(v => parseInt(v.trim()))
      await invoke('write_multiple_registers', {
        connectionId: selectedConnectionId.value,
        request: { address: writeForm.value.address, values }
      })
    } else if (fc === 'write_multiple_coils') {
      const values = writeForm.value.value.split(',').map(v => v.trim() === '1' || v.trim().toLowerCase() === 'true')
      await invoke('write_multiple_coils', {
        connectionId: selectedConnectionId.value,
        request: { address: writeForm.value.address, values }
      })
    }
    showWriteModal.value = false
  } catch (e) {
    await showAlert(String(e))
  }
}

const isConnected = () => selectedConnectionState.value === 'Connected'
const hasConnection = () => selectedConnectionId.value !== null
</script>

<template>
  <div class="toolbar">
    <div class="toolbar-group">
      <button class="toolbar-btn" @click="showNewConn = true">
        <span class="btn-icon">+</span> 新建连接
      </button>
    </div>

    <div class="toolbar-divider"></div>

    <div class="toolbar-group">
      <button class="toolbar-btn btn-start" :disabled="!hasConnection() || isConnected()" @click="connectMaster">
        连接
      </button>
      <button class="toolbar-btn btn-stop" :disabled="!hasConnection() || !isConnected()" @click="disconnectMaster">
        断开
      </button>
      <button class="toolbar-btn btn-close" :disabled="!hasConnection()" @click="deleteMaster">
        删除
      </button>
    </div>

    <div class="toolbar-divider"></div>

    <div class="toolbar-group">
      <button class="toolbar-btn" :disabled="!hasConnection()" @click="showNewScanGroup = true">
        <span class="btn-icon">+</span> 扫描组
      </button>
      <button class="toolbar-btn btn-start" :disabled="!hasConnection() || !isConnected()" @click="startAllPolling">
        全部启动
      </button>
      <button class="toolbar-btn btn-stop" :disabled="!hasConnection() || !isConnected()" @click="stopAllPolling">
        全部停止
      </button>
    </div>

    <div class="toolbar-divider"></div>

    <div class="toolbar-group">
      <button class="toolbar-btn" :disabled="!hasConnection() || !isConnected()" @click="showWriteModal = true">
        写入
      </button>
      <button class="toolbar-btn" :disabled="!hasConnection() || !isConnected()" @click="showScanDialog = true">
        扫描
      </button>
    </div>

    <div class="toolbar-spacer"></div>
    <span class="toolbar-title">ModbusMaster</span>
  </div>

  <!-- New Connection Modal -->
  <Teleport to="body">
    <div v-if="showNewConn" class="modal-backdrop" @click.self="showNewConn = false">
      <div class="modal-box">
        <div class="modal-title">新建连接</div>
        <div class="modal-body">
          <label class="form-label">
            目标地址
            <input v-model="newConnForm.target_address" class="form-input" type="text" placeholder="127.0.0.1" />
          </label>
          <label class="form-label">
            端口
            <input v-model.number="newConnForm.port" class="form-input" type="number" min="1" max="65535" />
          </label>
          <label class="form-label">
            从站 ID
            <input v-model.number="newConnForm.slave_id" class="form-input" type="number" min="1" max="247" />
          </label>
          <label class="form-label">
            超时 (ms)
            <input v-model.number="newConnForm.timeout_ms" class="form-input" type="number" min="100" max="30000" />
          </label>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="showNewConn = false">取消</button>
          <button class="btn btn-primary" @click="createConnection">创建</button>
        </div>
      </div>
    </div>
  </Teleport>

  <!-- Scan Group Modal -->
  <Teleport to="body">
    <div v-if="showNewScanGroup" class="modal-backdrop" @click.self="showNewScanGroup = false">
      <div class="modal-box">
        <div class="modal-title">新建扫描组</div>
        <div class="modal-body">
          <label class="form-label">
            名称
            <input v-model="scanGroupForm.name" class="form-input" type="text" placeholder="扫描组名称" />
          </label>
          <label class="form-label">
            功能码
            <select v-model="scanGroupForm.function" class="form-input">
              <option value="read_coils">FC01 - Read Coils</option>
              <option value="read_discrete_inputs">FC02 - Read Discrete Inputs</option>
              <option value="read_holding_registers">FC03 - Read Holding Registers</option>
              <option value="read_input_registers">FC04 - Read Input Registers</option>
            </select>
          </label>
          <label class="form-label">
            起始地址
            <input v-model.number="scanGroupForm.start_address" class="form-input" type="number" min="0" max="65535" />
          </label>
          <label class="form-label">
            数量
            <input v-model.number="scanGroupForm.quantity" class="form-input" type="number" min="1" max="125" />
          </label>
          <label class="form-label">
            轮询间隔 (ms)
            <input v-model.number="scanGroupForm.interval_ms" class="form-input" type="number" min="100" max="60000" />
          </label>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="showNewScanGroup = false">取消</button>
          <button class="btn btn-primary" @click="addScanGroup">创建</button>
        </div>
      </div>
    </div>
  </Teleport>

  <!-- Write Modal -->
  <Teleport to="body">
    <div v-if="showWriteModal" class="modal-backdrop" @click.self="showWriteModal = false">
      <div class="modal-box">
        <div class="modal-title">写入寄存器</div>
        <div class="modal-body">
          <label class="form-label">
            功能码
            <select v-model="writeForm.function" class="form-input">
              <option value="write_single_coil">FC05 - Write Single Coil</option>
              <option value="write_single_register">FC06 - Write Single Register</option>
              <option value="write_multiple_coils">FC15 - Write Multiple Coils</option>
              <option value="write_multiple_registers">FC16 - Write Multiple Registers</option>
            </select>
          </label>
          <label class="form-label">
            地址
            <input v-model.number="writeForm.address" class="form-input" type="number" min="0" max="65535" />
          </label>
          <label class="form-label">
            值 <span class="form-hint" v-if="writeForm.function.includes('multiple')">（逗号分隔）</span>
            <input v-model="writeForm.value" class="form-input" type="text" placeholder="0" />
          </label>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="showWriteModal = false">取消</button>
          <button class="btn btn-primary" @click="submitWrite">写入</button>
        </div>
      </div>
    </div>
  </Teleport>

  <ScanDialog v-if="showScanDialog" @close="showScanDialog = false" />
</template>

<style scoped>
.toolbar {
  display: flex;
  align-items: center;
  height: 42px;
  padding: 0 8px;
  gap: 0;
}

.toolbar-group {
  display: flex;
  gap: 2px;
}

.toolbar-divider {
  width: 1px;
  height: 20px;
  background: #313244;
  margin: 0 6px;
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

.btn-icon {
  font-weight: bold;
  font-size: 14px;
}

.btn-start { color: #a6e3a1; }
.btn-stop { color: #fab387; }
.btn-close { color: #f38ba8; }

.toolbar-spacer {
  flex: 1;
}

.toolbar-title {
  font-size: 13px;
  font-weight: 600;
  color: #6c7086;
  padding-right: 8px;
}

/* Modal */
.modal-backdrop {
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
  min-width: 340px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
}

.modal-title {
  font-size: 15px;
  font-weight: 600;
  color: #cdd6f4;
  margin-bottom: 16px;
}

.modal-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 20px;
}

.form-label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  color: #6c7086;
}

.form-input {
  padding: 6px 10px;
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  font-size: 13px;
}

.form-input:focus {
  outline: none;
  border-color: #89b4fa;
}

.form-hint {
  color: #6c7086;
  font-size: 11px;
}

.btn {
  padding: 7px 20px;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
}

.btn-primary {
  background: #89b4fa;
  color: #1e1e2e;
}

.btn-primary:hover {
  background: #74c7ec;
}

.btn-secondary {
  background: #45475a;
  color: #cdd6f4;
}

.btn-secondary:hover {
  background: #585b70;
}
</style>
