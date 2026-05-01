<script setup lang="ts">
import { computed, inject, ref, watch, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { save, open } from '@tauri-apps/plugin-dialog'
import { dialogKey } from '../composables/useDialog'
import type { showAlert as ShowAlert, showConfirm as ShowConfirm } from '../composables/useDialog'
import { float32ToU16Pair, type ByteOrder } from 'shared-frontend'
import ScanDialog from './ScanDialog.vue'

const { showAlert, showConfirm } = inject<{ showAlert: typeof ShowAlert; showConfirm: typeof ShowConfirm }>(dialogKey)!
const selectedConnectionId = inject<Ref<string | null>>('selectedConnectionId')!
const selectedConnectionState = inject<Ref<string>>('selectedConnectionState')!
const refreshTree = inject<() => void>('refreshTree')!

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

// New Connection modal
const showNewConn = ref(false)
const newConnForm = ref({
  transport: 'tcp',
  target_address: '127.0.0.1',
  port: 502,
  slave_id: 1,
  timeout_ms: 3000,
})
const serialPort = ref('')
const baudRate = ref(9600)
const dataBits = ref(8)
const stopBits = ref(1)
const parityMode = ref('none')
const serialPorts = ref<{ name: string; description: string; manufacturer: string }[]>([])

const useTls = ref(false)
const tlsCaFile = ref('')
const tlsCertFile = ref('')
const tlsKeyFile = ref('')
const tlsPkcs12File = ref('')
const tlsPkcs12Password = ref('')
const tlsAcceptInvalidCerts = ref(false)

async function pickFile(target: 'cert' | 'key' | 'ca' | 'pkcs12') {
  try {
    const path = await open({
      filters: target === 'pkcs12'
        ? [{ name: 'PKCS#12', extensions: ['p12', 'pfx'] }]
        : [{ name: 'PEM Certificate', extensions: ['pem', 'crt', 'key'] }],
    })
    if (!path) return
    const p = path as string
    if (target === 'cert') tlsCertFile.value = p
    else if (target === 'key') tlsKeyFile.value = p
    else if (target === 'ca') tlsCaFile.value = p
    else if (target === 'pkcs12') tlsPkcs12File.value = p
  } catch (e) {
    await showAlert(String(e))
  }
}

async function refreshSerialPorts() {
  try {
    serialPorts.value = await invoke('list_serial_ports')
  } catch (e) {
    await showAlert(String(e))
  }
}

watch(() => newConnForm.value.transport, (val) => {
  if (val === 'rtu' || val === 'ascii') {
    refreshSerialPorts()
  }
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
  dataType: 'raw' as 'raw' | 'float32',
  byteOrder: 'ABCD' as ByteOrder,
})

watch(() => writeForm.value.function, () => { writeForm.value.dataType = 'raw' })

const isMultiRegFC = computed(() => writeForm.value.function === 'write_multiple_registers')
const isFloat32Mode = computed(() => isMultiRegFC.value && writeForm.value.dataType === 'float32')

const float32Preview = computed(() => {
  if (!isFloat32Mode.value) return null
  const parts = writeForm.value.value.split(',').map(s => s.trim()).filter(s => s !== '')
  if (parts.length === 0) return null
  return parts.map((input, index) => {
    const n = parseFloat(input)
    if (isNaN(n)) return { index, input, float: null, regs: null, error: '无效数字' }
    const pair = float32ToU16Pair(n, writeForm.value.byteOrder)
    return { index, input, float: n, regs: pair, error: '' }
  })
})

const float32Valid = computed(() => {
  if (!float32Preview.value) return false
  return float32Preview.value.length > 0 && float32Preview.value.every(r => !r.error)
})

const float32RegCount = computed(() => {
  if (!float32Preview.value) return 0
  return float32Preview.value.filter(r => !r.error).length * 2
})

const float32Warning = computed(() => {
  if (!float32Preview.value || !float32Valid.value) return ''
  const count = float32Preview.value.length
  if (count * 2 > 123) return `超出 FC16 单次最大 123 寄存器限制 (当前 ${count * 2})`
  if (writeForm.value.address + count * 2 - 1 > 65535) return '寄存器地址溢出 (超过 65535)'
  return ''
})

async function createConnection() {
  const needsSerial = newConnForm.value.transport === 'rtu' || newConnForm.value.transport === 'ascii'
  if (needsSerial && !serialPort.value) {
    await showAlert('请选择串口')
    return
  }

  let transport: Record<string, unknown>
  if (newConnForm.value.transport === 'tcp') {
    transport = { type: 'tcp', host: newConnForm.value.target_address, port: newConnForm.value.port }
  } else if (newConnForm.value.transport === 'rtu' || newConnForm.value.transport === 'ascii') {
    transport = {
      type: newConnForm.value.transport,
      serial_port: serialPort.value,
      baud_rate: baudRate.value,
      data_bits: dataBits.value,
      stop_bits: stopBits.value,
      parity: parityMode.value,
    }
  } else {
    transport = { type: 'rtu_over_tcp', host: newConnForm.value.target_address, port: newConnForm.value.port }
  }

  if (useTls.value && newConnForm.value.transport === 'tcp') {
    transport = { type: 'tcp_tls', host: newConnForm.value.target_address, port: newConnForm.value.port }
  }

  try {
    await invoke('create_master_connection', {
      request: {
        transport,
        slave_id: newConnForm.value.slave_id,
        timeout_ms: newConnForm.value.timeout_ms,
        ...(useTls.value ? {
          use_tls: true,
          ca_file: tlsCaFile.value || undefined,
          cert_file: tlsCertFile.value || undefined,
          key_file: tlsKeyFile.value || undefined,
          pkcs12_file: tlsPkcs12File.value || undefined,
          pkcs12_password: tlsPkcs12Password.value || undefined,
          accept_invalid_certs: tlsAcceptInvalidCerts.value || undefined,
        } : {}),
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
    const doScan = await showConfirm('连接成功，是否扫描从站设备？')
    if (doScan) {
      showScanDialog.value = true
    }
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
      let values: number[]
      if (writeForm.value.dataType === 'float32') {
        const floats = writeForm.value.value.split(',').map(v => parseFloat(v.trim()))
        if (floats.some(isNaN)) { await showAlert('包含无效的浮点数'); return }
        values = []
        for (const f of floats) {
          const [r0, r1] = float32ToU16Pair(f, writeForm.value.byteOrder)
          values.push(r0, r1)
        }
      } else {
        values = writeForm.value.value.split(',').map(v => parseInt(v.trim()))
      }
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
const isReconnecting = () => selectedConnectionState.value === 'Reconnecting'
const isDisconnected = () => selectedConnectionState.value === 'Disconnected'
const hasConnection = () => selectedConnectionId.value !== null
</script>

<template>
  <div class="toolbar">
    <div class="toolbar-group">
      <button class="toolbar-btn" @click="openProject" title="打开项目">打开</button>
      <button class="toolbar-btn" @click="saveProject" title="保存项目">保存</button>
      <button class="toolbar-btn" @click="saveProjectAs" title="另存为">另存为</button>
    </div>

    <div class="toolbar-divider"></div>

    <div class="toolbar-group">
      <button class="toolbar-btn" @click="showNewConn = true">
        <span class="btn-icon">+</span> 新建连接
      </button>
    </div>

    <div class="toolbar-divider"></div>

    <div class="toolbar-group">
      <button class="toolbar-btn btn-start" :disabled="!hasConnection() || isConnected() || isReconnecting()" @click="connectMaster">
        连接
      </button>
      <button class="toolbar-btn btn-stop" :disabled="!hasConnection() || isDisconnected()" @click="disconnectMaster">
        {{ isReconnecting() ? '取消重连' : '断开' }}
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
            传输类型
            <select v-model="newConnForm.transport" class="form-input">
              <option value="tcp">TCP</option>
              <option value="rtu">RTU (串口)</option>
              <option value="ascii">ASCII (串口)</option>
              <option value="rtu_over_tcp">RTU over TCP</option>
            </select>
          </label>
          <template v-if="newConnForm.transport === 'tcp' || newConnForm.transport === 'rtu_over_tcp'">
            <label class="form-label">
              目标地址
              <input v-model="newConnForm.target_address" class="form-input" type="text" placeholder="127.0.0.1" />
            </label>
            <label class="form-label">
              端口
              <input v-model.number="newConnForm.port" class="form-input" type="number" min="1" max="65535" />
            </label>
          </template>
          <template v-if="newConnForm.transport === 'tcp'">
            <label class="form-label">
              <input type="checkbox" v-model="useTls" /> 启用 TLS
            </label>
            <template v-if="useTls">
              <label class="form-label">
                CA 证书 (验证服务器)
                <div style="display: flex; gap: 4px;">
                  <input v-model="tlsCaFile" class="form-input" type="text" placeholder="CA 证书路径" style="flex: 1;" />
                  <button class="tool-btn" @click="pickFile('ca')" style="padding: 4px 8px;">...</button>
                </div>
              </label>
              <label class="form-label">
                客户端证书 (PEM)
                <div style="display: flex; gap: 4px;">
                  <input v-model="tlsCertFile" class="form-input" type="text" placeholder="可选，用于 mTLS" style="flex: 1;" />
                  <button class="tool-btn" @click="pickFile('cert')" style="padding: 4px 8px;">...</button>
                </div>
              </label>
              <label class="form-label">
                客户端私钥 (PEM)
                <div style="display: flex; gap: 4px;">
                  <input v-model="tlsKeyFile" class="form-input" type="text" placeholder="可选，用于 mTLS" style="flex: 1;" />
                  <button class="tool-btn" @click="pickFile('key')" style="padding: 4px 8px;">...</button>
                </div>
              </label>
              <label class="form-label">
                PKCS#12 文件
                <div style="display: flex; gap: 4px;">
                  <input v-model="tlsPkcs12File" class="form-input" type="text" placeholder="可选，优先于 PEM" style="flex: 1;" />
                  <button class="tool-btn" @click="pickFile('pkcs12')" style="padding: 4px 8px;">...</button>
                </div>
              </label>
              <label class="form-label" v-if="tlsPkcs12File">
                PKCS#12 密码
                <input v-model="tlsPkcs12Password" class="form-input" type="password" placeholder="密码" />
              </label>
              <label class="form-label">
                <input type="checkbox" v-model="tlsAcceptInvalidCerts" /> 接受自签名证书 (测试用)
              </label>
            </template>
          </template>
          <template v-if="newConnForm.transport === 'rtu' || newConnForm.transport === 'ascii'">
            <label class="form-label">
              串口
              <div style="display: flex; gap: 4px;">
                <select v-model="serialPort" class="form-input" style="flex: 1;">
                  <option v-for="p in serialPorts" :key="p.name" :value="p.name">
                    {{ p.name }}{{ p.description ? ` (${p.description})` : '' }}
                  </option>
                </select>
                <button class="tool-btn" @click="refreshSerialPorts" title="刷新串口列表" style="padding: 4px 8px;">&#x21bb;</button>
              </div>
            </label>
            <label class="form-label">
              波特率
              <select v-model.number="baudRate" class="form-input">
                <option :value="9600">9600</option>
                <option :value="19200">19200</option>
                <option :value="38400">38400</option>
                <option :value="57600">57600</option>
                <option :value="115200">115200</option>
              </select>
            </label>
            <label class="form-label">
              数据位
              <select v-model.number="dataBits" class="form-input">
                <option :value="7">7</option>
                <option :value="8">8</option>
              </select>
            </label>
            <label class="form-label">
              停止位
              <select v-model.number="stopBits" class="form-input">
                <option :value="1">1</option>
                <option :value="2">2</option>
              </select>
            </label>
            <label class="form-label">
              校验
              <select v-model="parityMode" class="form-input">
                <option value="none">None</option>
                <option value="odd">Odd</option>
                <option value="even">Even</option>
              </select>
            </label>
          </template>
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
          <label v-if="isMultiRegFC" class="form-label">
            数据类型
            <select v-model="writeForm.dataType" class="form-input">
              <option value="raw">Raw u16</option>
              <option value="float32">Float32 (REAL)</option>
            </select>
          </label>
          <label v-if="isFloat32Mode" class="form-label">
            字节序
            <select v-model="writeForm.byteOrder" class="form-input">
              <option value="ABCD">AB CD (Big Endian)</option>
              <option value="CDAB">CD AB (Little Endian Word Swap)</option>
              <option value="BADC">BA DC (Byte Swap)</option>
              <option value="DCBA">DC BA (Little Endian)</option>
            </select>
          </label>
          <label class="form-label">
            地址
            <input v-model.number="writeForm.address" class="form-input" type="number" min="0" max="65535" />
          </label>
          <label class="form-label">
            值
            <span class="form-hint" v-if="isFloat32Mode">（逗号分隔浮点数，如 3.14, 2.71, 1.41）</span>
            <span class="form-hint" v-else-if="writeForm.function.includes('multiple')">（逗号分隔）</span>
            <textarea v-if="isFloat32Mode" v-model="writeForm.value" class="form-input form-textarea"
              placeholder="3.14, 2.71, 1.41" rows="3" />
            <input v-else v-model="writeForm.value" class="form-input" type="text" placeholder="0" />
          </label>
          <div v-if="float32Preview && float32Preview.length > 0" class="float-preview">
            <div class="preview-summary">
              {{ float32Preview.length }} 个 Float32 = {{ float32RegCount }} 个寄存器，起始地址 {{ writeForm.address }}
              <span v-if="float32Warning" class="preview-warn">{{ float32Warning }}</span>
            </div>
            <table class="preview-table">
              <thead><tr><th>地址</th><th>Float</th><th>Reg[0]</th><th>Reg[1]</th></tr></thead>
              <tbody>
                <tr v-for="item in float32Preview" :key="item.index" :class="{ 'preview-error': item.error }">
                  <td>{{ writeForm.address + item.index * 2 }}</td>
                  <td>{{ item.error || item.float }}</td>
                  <td>{{ item.regs ? '0x' + item.regs[0].toString(16).toUpperCase().padStart(4, '0') : '-' }}</td>
                  <td>{{ item.regs ? '0x' + item.regs[1].toString(16).toUpperCase().padStart(4, '0') : '-' }}</td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="showWriteModal = false">取消</button>
          <button class="btn btn-primary" @click="submitWrite"
            :disabled="isFloat32Mode && (!float32Valid || !!float32Warning)">写入</button>
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

.tool-btn {
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 4px;
  color: #cdd6f4;
  cursor: pointer;
  font-size: 14px;
}

.tool-btn:hover {
  background: #45475a;
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

.btn-primary:disabled {
  opacity: 0.4;
  cursor: default;
}

.form-textarea {
  resize: vertical;
  min-height: 60px;
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.float-preview {
  margin-top: 4px;
  border: 1px solid #313244;
  border-radius: 4px;
  overflow: hidden;
}

.preview-summary {
  padding: 4px 8px;
  font-size: 11px;
  color: #a6e3a1;
  background: #181825;
}

.preview-warn {
  color: #fab387;
  margin-left: 8px;
}

.preview-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 11px;
}

.preview-table th {
  background: #181825;
  color: #6c7086;
  font-weight: 500;
  padding: 3px 8px;
  text-align: left;
}

.preview-table td {
  padding: 2px 8px;
  color: #cdd6f4;
  font-family: 'SF Mono', 'Fira Code', monospace;
  border-top: 1px solid #1e1e2e;
}

.preview-error td {
  color: #f38ba8;
}
</style>
