<script setup lang="ts">
import { ref, inject, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { save, open } from '@tauri-apps/plugin-dialog'
import { useI18n, LangToggle, showAlert, showConfirm } from 'shared-frontend'
import NewConnectionDialog from './NewConnectionDialog.vue'
import NewSlaveDialog from './NewSlaveDialog.vue'
import MutationControl from './MutationControl.vue'

const { t } = useI18n()

const selectedConnectionId = inject<Ref<string | null>>('selectedConnectionId')!
const selectedConnectionState = inject<Ref<string>>('selectedConnectionState')!
const selectedSlaveId = inject<Ref<number | null>>('selectedSlaveId')!
const refreshTree = inject<() => void>('refreshTree')!
const refreshRegisters = inject<() => void>('refreshRegisters')!

const currentProjectPath = ref<string | null>(null)
const showNewConn = ref(false)
const showNewSlave = ref(false)

async function openProject() {
  try {
    const path = await open({
      filters: [{ name: 'Modbus Project', extensions: ['modbusproj'] }],
    })
    if (!path) return
    await invoke('load_project_file', { path })
    currentProjectPath.value = path as string
    refreshTree()
  } catch (e) { await showAlert(String(e)) }
}

async function saveProject() {
  if (!currentProjectPath.value) return saveProjectAs()
  try {
    await invoke('save_project_file', { path: currentProjectPath.value })
  } catch (e) { await showAlert(String(e)) }
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
  } catch (e) { await showAlert(String(e)) }
}

async function startConnection() {
  if (!selectedConnectionId.value) return
  try {
    await invoke('start_slave_connection', { id: selectedConnectionId.value })
    selectedConnectionState.value = 'Running'
    refreshTree()
  } catch (e) { await showAlert(String(e)) }
}

async function stopConnection() {
  if (!selectedConnectionId.value) return
  try {
    await invoke('stop_slave_connection', { id: selectedConnectionId.value })
    selectedConnectionState.value = 'Stopped'
    refreshTree()
  } catch (e) { await showAlert(String(e)) }
}

async function closeConnection() {
  if (!selectedConnectionId.value) return
  if (!(await showConfirm(t('errors.confirmDeleteConnection')))) return
  try {
    await invoke('delete_slave_connection', { id: selectedConnectionId.value })
    selectedConnectionId.value = null
    selectedConnectionState.value = 'Stopped'
    refreshTree()
  } catch (e) { await showAlert(String(e)) }
}

async function openTools() {
  await showAlert(t('errors.toolPanelPending'))
}
</script>

<template>
  <div class="toolbar">
    <div class="toolbar-group">
      <button class="toolbar-btn" @click="openProject" :title="t('toolbar.openProjectTitle')">
        <span class="toolbar-label">{{ t('toolbar.open') }}</span>
      </button>
      <button class="toolbar-btn" @click="saveProject" :title="t('toolbar.saveProjectTitle')">
        <span class="toolbar-label">{{ t('common.save') }}</span>
      </button>
      <button class="toolbar-btn" @click="saveProjectAs" :title="t('toolbar.saveAsTitle')">
        <span class="toolbar-label">{{ t('toolbar.saveAs') }}</span>
      </button>
    </div>
    <div class="toolbar-divider"></div>
    <div class="toolbar-group">
      <button class="toolbar-btn" @click="showNewConn = true" :title="t('toolbar.newConnection')">
        <span class="toolbar-icon">+</span>
        <span class="toolbar-label">{{ t('toolbar.newConnection') }}</span>
      </button>
      <button
        class="toolbar-btn"
        @click="showNewSlave = true"
        :disabled="!selectedConnectionId"
        :title="t('toolbar.newSlave')"
      >
        <span class="toolbar-icon">+</span>
        <span class="toolbar-label">{{ t('toolbar.newSlave') }}</span>
      </button>
    </div>
    <div class="toolbar-divider"></div>
    <div class="toolbar-group">
      <button
        class="toolbar-btn btn-start"
        @click="startConnection"
        :disabled="!selectedConnectionId || selectedConnectionState === 'Running'"
        :title="t('toolbar.startConnection')"
      >
        <span class="toolbar-label">{{ t('toolbar.start') }}</span>
      </button>
      <button
        class="toolbar-btn btn-stop"
        @click="stopConnection"
        :disabled="!selectedConnectionId || selectedConnectionState === 'Stopped'"
        :title="t('toolbar.stopConnection')"
      >
        <span class="toolbar-label">{{ t('toolbar.stop') }}</span>
      </button>
      <button
        class="toolbar-btn btn-close"
        @click="closeConnection"
        :disabled="!selectedConnectionId"
        :title="t('toolbar.closeConnection')"
      >
        <span class="toolbar-label">{{ t('common.close') }}</span>
      </button>
    </div>
    <div class="toolbar-divider"></div>
    <MutationControl
      :connection-id="selectedConnectionId"
      :slave-id="selectedSlaveId"
      @mutated="refreshRegisters"
    />
    <div class="toolbar-divider"></div>
    <div class="toolbar-group">
      <button class="toolbar-btn" @click="openTools" :title="t('common.tools')">
        <span class="toolbar-label">{{ t('common.tools') }}</span>
      </button>
    </div>
    <div class="toolbar-spacer" style="flex:1"></div>
    <LangToggle />
    <div class="toolbar-title">{{ t('toolbar.appTitleSlave') }}</div>
  </div>

  <NewConnectionDialog :show="showNewConn" @close="showNewConn = false" @created="refreshTree" />
  <NewSlaveDialog
    :show="showNewSlave"
    :connection-id="selectedConnectionId"
    @close="showNewSlave = false"
    @created="refreshTree"
  />
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

.toolbar-group { display: flex; gap: 2px; }
.toolbar-divider { width: 1px; height: 24px; background: #313244; margin: 0 4px; }
.toolbar-btn {
  display: flex; align-items: center; gap: 4px;
  padding: 4px 10px; border: none; background: transparent;
  color: #cdd6f4; cursor: pointer; border-radius: 4px;
  font-size: 12px; white-space: nowrap;
}
.toolbar-btn:hover:not(:disabled) { background: #313244; }
.toolbar-btn:disabled { opacity: 0.4; cursor: default; }
.toolbar-btn.btn-start:not(:disabled) { color: #a6e3a1; }
.toolbar-btn.btn-stop:not(:disabled) { color: #fab387; }
.toolbar-btn.btn-close:not(:disabled) { color: #f38ba8; }
.toolbar-icon { font-weight: bold; font-size: 14px; }
.toolbar-title { margin-left: auto; font-size: 12px; color: #6c7086; padding-right: 8px; }
</style>
