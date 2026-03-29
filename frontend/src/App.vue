<script setup lang="ts">
import { ref, provide } from 'vue'
import Toolbar from './components/Toolbar.vue'
import ConnectionTree from './components/ConnectionTree.vue'
import RegisterTable from './components/RegisterTable.vue'
import ValuePanel from './components/ValuePanel.vue'
import LogPanel from './components/LogPanel.vue'
import AppDialog from './components/AppDialog.vue'
import { showAlert, showConfirm, showPrompt, dialogKey } from './composables/useDialog'

// Shared state
const selectedConnectionId = ref<string | null>(null)
const selectedConnectionState = ref<string>('Stopped')
const selectedSlaveId = ref<number | null>(null)
const selectedRegisterType = ref<string | null>(null)
const selectedRegister = ref<{ address: number; register_type: string; value: number }[]>([])
const logExpanded = ref(false)

// Provide shared state to children
provide('selectedConnectionId', selectedConnectionId)
provide('selectedConnectionState', selectedConnectionState)
provide('selectedSlaveId', selectedSlaveId)
provide('selectedRegisterType', selectedRegisterType)
provide('selectedRegister', selectedRegister)

// Tree refresh trigger
const treeRefreshKey = ref(0)
provide('treeRefreshKey', treeRefreshKey)

function refreshTree() {
  treeRefreshKey.value++
}

provide('refreshTree', refreshTree)

// Register refresh trigger (for ValuePanel → RegisterTable sync)
const registerRefreshKey = ref(0)
provide('registerRefreshKey', registerRefreshKey)

function refreshRegisters() {
  registerRefreshKey.value++
}

provide('refreshRegisters', refreshRegisters)
provide(dialogKey, { showAlert, showConfirm, showPrompt })

function handleConnectionSelect(id: string, state: string) {
  selectedConnectionId.value = id
  selectedConnectionState.value = state
  selectedSlaveId.value = null
  selectedRegisterType.value = null
  selectedRegister.value = []
}

function handleSlaveSelect(connectionId: string, slaveId: number) {
  selectedConnectionId.value = connectionId
  selectedSlaveId.value = slaveId
  selectedRegisterType.value = null
  selectedRegister.value = []
}

function handleGroupSelect(connectionId: string, slaveId: number, regType: string) {
  selectedConnectionId.value = connectionId
  selectedSlaveId.value = slaveId
  selectedRegisterType.value = regType
  selectedRegister.value = []
}

function handleRegisterSelect(regs: { address: number; register_type: string; value: number }[]) {
  selectedRegister.value = regs
}

function toggleLog() {
  logExpanded.value = !logExpanded.value
}
</script>

<template>
  <div :class="['app-layout', { 'log-expanded': logExpanded }]">
    <header class="toolbar-area">
      <Toolbar />
    </header>

    <aside class="tree-area">
      <ConnectionTree
        @connection-select="handleConnectionSelect"
        @slave-select="handleSlaveSelect"
        @group-select="handleGroupSelect"
      />
    </aside>
    <main class="content-area">
      <RegisterTable
        @register-select="handleRegisterSelect"
      />
    </main>
    <aside class="panel-area">
      <ValuePanel />
    </aside>

    <footer class="log-area">
      <LogPanel :expanded="logExpanded" @toggle="toggleLog" />
    </footer>
    <AppDialog />
  </div>
</template>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body, #app {
  height: 100%;
  width: 100%;
  overflow: hidden;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
  background: #11111b;
  color: #cdd6f4;
}

.app-layout {
  display: grid;
  grid-template-columns: 240px 1fr 280px;
  grid-template-rows: 42px 1fr 32px;
  grid-template-areas:
    "toolbar toolbar toolbar"
    "tree content panel"
    "log log log";
  height: 100vh;
  width: 100vw;
}

.app-layout.log-expanded {
  grid-template-rows: 42px 1fr 200px;
}

.toolbar-area {
  grid-area: toolbar;
  background: #1e1e2e;
  border-bottom: 1px solid #313244;
}

.tree-area {
  grid-area: tree;
  background: #181825;
  border-right: 1px solid #313244;
  overflow-y: auto;
}

.content-area {
  grid-area: content;
  background: #11111b;
  overflow: hidden;
}

.panel-area {
  grid-area: panel;
  background: #181825;
  border-left: 1px solid #313244;
  overflow-y: auto;
}

.log-area {
  grid-area: log;
  background: #1e1e2e;
  border-top: 1px solid #313244;
  overflow: hidden;
}
</style>
