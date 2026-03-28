<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// Address Conversion
const addressInput = ref(40001)
const addressResult = ref<string | null>(null)
const addressError = ref<string | null>(null)

// CRC/LRC
const hexInput = ref('01 03 00 00 00 0A')
const crcResult = ref<string | null>(null)
const lrcResult = ref<string | null>(null)
const crcError = ref<string | null>(null)

async function convertAddress() {
  addressError.value = null
  addressResult.value = null
  try {
    const result = await invoke<{
      plc_address: number
      protocol_address: number
      register_type: string
    }>('convert_plc_to_modbus', {
      request: { address: addressInput.value }
    })
    addressResult.value = `PLC: ${result.plc_address} → Protocol: ${result.protocol_address} (${result.register_type})`
  } catch (e) {
    addressError.value = String(e)
  }
}

async function calculateChecksum() {
  crcError.value = null
  crcResult.value = null
  lrcResult.value = null
  try {
    const [crc, lrc] = await Promise.all([
      invoke<string>('calculate_crc16', { data: hexInput.value }),
      invoke<string>('calculate_lrc', { data: hexInput.value })
    ])
    crcResult.value = crc
    lrcResult.value = lrc
  } catch (e) {
    crcError.value = String(e)
  }
}
</script>

<template>
  <div class="tools-view">
    <div class="panel-header">
      <h2>Tools</h2>
    </div>

    <div class="panel-content">
      <!-- Address Conversion -->
      <div class="tool-section">
        <h3>Address Conversion</h3>
        <p class="tool-description">
          Convert between Modbus PLC addresses and protocol addresses.
        </p>
        <div class="tool-form">
          <input
            v-model.number="addressInput"
            type="number"
            placeholder="PLC Address (e.g. 40001)"
            class="input-large"
          />
          <button class="btn btn-primary" @click="convertAddress">Convert</button>
        </div>
        <div v-if="addressResult" class="tool-result">
          {{ addressResult }}
        </div>
        <div v-if="addressError" class="tool-error">{{ addressError }}</div>
        <div class="tool-info">
          <p><strong>Address Types:</strong></p>
          <ul>
            <li>0xxxx = Coil (FC01/05/15)</li>
            <li>1xxxx = Discrete Input (FC02)</li>
            <li>3xxxx = Input Register (FC04)</li>
            <li>4xxxx = Holding Register (FC03/06/16)</li>
          </ul>
        </div>
      </div>

      <!-- CRC/LRC Calculator -->
      <div class="tool-section">
        <h3>Checksum Calculator</h3>
        <p class="tool-description">
          Calculate CRC-16 (Modbus RTU) and LRC (Modbus ASCII) checksums.
        </p>
        <div class="tool-form">
          <input
            v-model="hexInput"
            type="text"
            placeholder="Hex data (e.g. 01 03 00 00 00 0A)"
            class="input-large"
          />
          <button class="btn btn-primary" @click="calculateChecksum">Calculate</button>
        </div>
        <div v-if="crcResult !== null" class="tool-result">
          <div><strong>CRC-16:</strong> {{ crcResult }}</div>
          <div><strong>LRC:</strong> {{ lrcResult }}</div>
        </div>
        <div v-if="crcError" class="tool-error">{{ crcError }}</div>
        <div class="tool-info">
          <p><strong>Supported Formats:</strong></p>
          <ul>
            <li>Space separated: "01 02 03 04"</li>
            <li>Comma separated: "01,02,03,04"</li>
            <li>No separator: "01020304"</li>
            <li>Mixed: "01 02,03 04"</li>
          </ul>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.tools-view {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.panel-header {
  padding: 16px;
  border-bottom: 1px solid #313244;
}

.panel-header h2 {
  margin: 0;
}

.panel-content {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.tool-section {
  margin-bottom: 32px;
  padding: 16px;
  background: #1e1e2e;
  border-radius: 8px;
}

.tool-section h3 {
  margin: 0 0 8px 0;
  font-size: 16px;
}

.tool-description {
  color: #6c7086;
  font-size: 13px;
  margin: 0 0 16px 0;
}

.tool-form {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}

.tool-result {
  padding: 12px;
  background: #313244;
  border-radius: 6px;
  font-family: monospace;
  font-size: 14px;
  margin-bottom: 12px;
}

.tool-result div {
  margin: 4px 0;
}

.tool-error {
  padding: 12px;
  background: #f38ba8;
  color: #1e1e2e;
  border-radius: 6px;
  font-size: 13px;
  margin-bottom: 12px;
}

.tool-info {
  padding: 12px;
  background: #313244;
  border-radius: 6px;
  font-size: 13px;
}

.tool-info p {
  margin: 0 0 8px 0;
}

.tool-info ul {
  margin: 0;
  padding-left: 20px;
}

.tool-info li {
  margin: 4px 0;
  color: #6c7086;
}

.input-large {
  flex: 1;
  padding: 10px 14px;
  background: #1e1e2e;
  border: 1px solid #45475a;
  border-radius: 6px;
  color: #cdd6f4;
  font-size: 14px;
}

.btn {
  padding: 10px 20px;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
}

.btn-primary {
  background: #89b4fa;
  color: #1e1e2e;
}
</style>
