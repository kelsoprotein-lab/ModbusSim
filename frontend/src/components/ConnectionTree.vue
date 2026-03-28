<script setup lang="ts">
import { ref, inject, watch, onMounted, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { dialogKey } from '../composables/useDialog'
import type { showAlert as ShowAlert } from '../composables/useDialog'

const { showAlert } = inject<{ showAlert: typeof ShowAlert }>(dialogKey)!

interface SlaveConnection {
  id: string
  bind_address: string
  port: number
  state: string
  device_count: number
}

interface SlaveDevice {
  slave_id: number
  name: string
  register_count: number
}

interface TreeConnection {
  conn: SlaveConnection
  expanded: boolean
  devices: TreeDevice[]
}

interface TreeDevice {
  device: SlaveDevice
  expanded: boolean
  connectionId: string
}

const REGISTER_GROUPS = [
  { type: 'coil', label: '0x Coils' },
  { type: 'discrete_input', label: '1x Discrete Inputs' },
  { type: 'input_register', label: '3x Input Registers' },
  { type: 'holding_register', label: '4x Holding Registers' },
]

const emit = defineEmits<{
  (e: 'connection-select', id: string, state: string): void
  (e: 'slave-select', connectionId: string, slaveId: number): void
  (e: 'group-select', connectionId: string, slaveId: number, regType: string): void
}>()

const treeRefreshKey = inject<Ref<number>>('treeRefreshKey')!
const selectedConnectionId = inject<Ref<string | null>>('selectedConnectionId')!
const selectedSlaveId = inject<Ref<number | null>>('selectedSlaveId')!
const selectedRegisterType = inject<Ref<string | null>>('selectedRegisterType')!

const treeData = ref<TreeConnection[]>([])
const contextMenu = ref({ show: false, x: 0, y: 0, type: '' as 'connection' | 'slave', connectionId: '', slaveId: 0, connState: '' })

async function loadTree() {
  try {
    const connections = await invoke<SlaveConnection[]>('list_slave_connections')
    const newTree: TreeConnection[] = []

    for (const conn of connections) {
      const existing = treeData.value.find(t => t.conn.id === conn.id)
      const devices = await invoke<SlaveDevice[]>('list_slave_devices', { connectionId: conn.id })
      newTree.push({
        conn,
        expanded: existing ? existing.expanded : true,
        devices: devices.map(d => ({
          device: d,
          expanded: existing?.devices.find(ed => ed.device.slave_id === d.slave_id)?.expanded ?? true,
          connectionId: conn.id,
        })),
      })
    }
    treeData.value = newTree
  } catch (e) {
    console.error('Failed to load tree:', e)
  }
}

watch(treeRefreshKey, () => loadTree())
onMounted(loadTree)

function toggleConnection(tc: TreeConnection) {
  tc.expanded = !tc.expanded
}

function toggleDevice(td: TreeDevice) {
  td.expanded = !td.expanded
}

function selectConnection(tc: TreeConnection) {
  emit('connection-select', tc.conn.id, tc.conn.state)
}

function selectSlave(tc: TreeConnection, td: TreeDevice) {
  emit('slave-select', tc.conn.id, td.device.slave_id)
}

function selectGroup(tc: TreeConnection, td: TreeDevice, regType: string) {
  emit('group-select', tc.conn.id, td.device.slave_id, regType)
}

function showContextMenuForConnection(e: MouseEvent, tc: TreeConnection) {
  e.preventDefault()
  contextMenu.value = {
    show: true,
    x: e.clientX,
    y: e.clientY,
    type: 'connection',
    connectionId: tc.conn.id,
    slaveId: 0,
    connState: tc.conn.state,
  }
}

function showContextMenuForSlave(e: MouseEvent, tc: TreeConnection, td: TreeDevice) {
  e.preventDefault()
  contextMenu.value = {
    show: true,
    x: e.clientX,
    y: e.clientY,
    type: 'slave',
    connectionId: tc.conn.id,
    slaveId: td.device.slave_id,
    connState: '',
  }
}

function closeContextMenu() {
  contextMenu.value.show = false
}

async function ctxStartConnection() {
  closeContextMenu()
  try {
    await invoke('start_slave_connection', { id: contextMenu.value.connectionId })
    await loadTree()
  } catch (e) { await showAlert(String(e)) }
}

async function ctxStopConnection() {
  closeContextMenu()
  try {
    await invoke('stop_slave_connection', { id: contextMenu.value.connectionId })
    await loadTree()
  } catch (e) { await showAlert(String(e)) }
}

async function ctxDeleteConnection() {
  closeContextMenu()
  try {
    await invoke('delete_slave_connection', { id: contextMenu.value.connectionId })
    if (selectedConnectionId.value === contextMenu.value.connectionId) {
      selectedConnectionId.value = null
    }
    await loadTree()
  } catch (e) { await showAlert(String(e)) }
}

async function ctxDeleteSlave() {
  closeContextMenu()
  try {
    await invoke('remove_slave_device', {
      connectionId: contextMenu.value.connectionId,
      slaveId: contextMenu.value.slaveId,
    })
    await loadTree()
  } catch (e) { await showAlert(String(e)) }
}
</script>

<template>
  <div class="connection-tree" @click="closeContextMenu">
    <div class="tree-header">连接</div>
    <div v-if="treeData.length === 0" class="tree-empty">暂无连接</div>

    <div v-for="tc in treeData" :key="tc.conn.id" class="tree-node-group">
      <!-- Connection Node -->
      <div
        :class="['tree-node connection-node', { selected: tc.conn.id === selectedConnectionId && selectedSlaveId === null }]"
        @click.stop="selectConnection(tc)"
        @contextmenu.prevent="showContextMenuForConnection($event, tc)"
      >
        <span class="node-arrow" @click.stop="toggleConnection(tc)">{{ tc.expanded ? '▼' : '▶' }}</span>
        <span :class="['node-status', tc.conn.state === 'Running' ? 'running' : 'stopped']"></span>
        <span class="node-label">{{ tc.conn.bind_address }}:{{ tc.conn.port }}</span>
      </div>

      <!-- Slave Nodes -->
      <template v-if="tc.expanded">
        <div v-for="td in tc.devices" :key="td.device.slave_id" class="tree-child">
          <div
            :class="['tree-node slave-node', { selected: tc.conn.id === selectedConnectionId && td.device.slave_id === selectedSlaveId && selectedRegisterType === null }]"
            @click.stop="selectSlave(tc, td)"
            @contextmenu.prevent="showContextMenuForSlave($event, tc, td)"
          >
            <span class="node-arrow" @click.stop="toggleDevice(td)">{{ td.expanded ? '▼' : '▶' }}</span>
            <span class="node-label">{{ td.device.name || `从站 ${td.device.slave_id}` }}</span>
          </div>

          <!-- Register Group Nodes -->
          <template v-if="td.expanded">
            <div
              v-for="group in REGISTER_GROUPS"
              :key="group.type"
              :class="['tree-node group-node', { selected: tc.conn.id === selectedConnectionId && td.device.slave_id === selectedSlaveId && selectedRegisterType === group.type }]"
              @click.stop="selectGroup(tc, td, group.type)"
            >
              <span class="node-label">{{ group.label }}</span>
            </div>
          </template>
        </div>
      </template>
    </div>

    <!-- Context Menu -->
    <div
      v-if="contextMenu.show"
      class="context-menu"
      :style="{ top: contextMenu.y + 'px', left: contextMenu.x + 'px' }"
      @click.stop
    >
      <template v-if="contextMenu.type === 'connection'">
        <div
          v-if="contextMenu.connState === 'Stopped'"
          class="context-menu-item"
          @click="ctxStartConnection"
        >启动连接</div>
        <div
          v-else
          class="context-menu-item"
          @click="ctxStopConnection"
        >停止连接</div>
        <div class="context-menu-item danger" @click="ctxDeleteConnection">删除连接</div>
      </template>
      <template v-if="contextMenu.type === 'slave'">
        <div class="context-menu-item danger" @click="ctxDeleteSlave">删除从站</div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.connection-tree {
  padding: 0;
  font-size: 13px;
  user-select: none;
  height: 100%;
  position: relative;
}

.tree-header {
  padding: 8px 12px;
  font-size: 11px;
  text-transform: uppercase;
  color: #6c7086;
  letter-spacing: 0.5px;
}

.tree-empty {
  padding: 16px 12px;
  color: #6c7086;
  font-size: 12px;
}

.tree-node {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 8px;
  cursor: pointer;
  border-radius: 3px;
  margin: 1px 4px;
}

.tree-node:hover {
  background: #313244;
}

.tree-node.selected {
  background: #89b4fa;
  color: #1e1e2e;
}

.tree-child {
  padding-left: 16px;
}

.group-node {
  padding-left: 44px;
}

.node-arrow {
  font-size: 8px;
  width: 12px;
  text-align: center;
  flex-shrink: 0;
  color: #6c7086;
}

.tree-node.selected .node-arrow {
  color: #1e1e2e;
}

.node-status {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.node-status.running {
  background: #a6e3a1;
}

.node-status.stopped {
  background: #f38ba8;
}

.node-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
}

.context-menu-item:first-child {
  border-radius: 6px 6px 0 0;
}

.context-menu-item:last-child {
  border-radius: 0 0 6px 6px;
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
