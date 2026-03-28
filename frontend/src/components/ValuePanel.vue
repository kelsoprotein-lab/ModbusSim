<script setup lang="ts">
import { inject, computed, ref, watch, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const selectedConnectionId = inject<Ref<string | null>>('selectedConnectionId')!
const selectedSlaveId = inject<Ref<number | null>>('selectedSlaveId')!
const selectedRegister = inject<Ref<{ address: number; register_type: string; value: number } | null>>('selectedRegister')!

const neighborValues = ref<number[]>([])

const isBoolType = computed(() => {
  if (!selectedRegister.value) return false
  return selectedRegister.value.register_type === 'coil' || selectedRegister.value.register_type === 'discrete_input'
})

const panelTitle = computed(() => {
  if (!selectedRegister.value) return ''
  const reg = selectedRegister.value
  const prefixMap: Record<string, string> = {
    coil: '0x Coil',
    discrete_input: '1x Discrete Input',
    input_register: '3x Input Register',
    holding_register: '4x Holding Register',
  }
  const prefix = prefixMap[reg.register_type] || reg.register_type
  return `${prefix} @ 0x${reg.address.toString(16).toUpperCase().padStart(4, '0')}`
})

// 16-bit interpretations
const signed16 = computed(() => {
  if (!selectedRegister.value) return 0
  const v = selectedRegister.value.value & 0xFFFF
  return v >= 0x8000 ? v - 0x10000 : v
})

const unsigned16 = computed(() => {
  if (!selectedRegister.value) return 0
  return selectedRegister.value.value & 0xFFFF
})

const hex16 = computed(() => {
  if (!selectedRegister.value) return '0x0000'
  return '0x' + (selectedRegister.value.value & 0xFFFF).toString(16).toUpperCase().padStart(4, '0')
})

const binary16 = computed(() => {
  if (!selectedRegister.value) return '0000 0000 0000 0000'
  const b = (selectedRegister.value.value & 0xFFFF).toString(2).padStart(16, '0')
  return `${b.slice(0, 4)} ${b.slice(4, 8)} ${b.slice(8, 12)} ${b.slice(12, 16)}`
})

// 32-bit interpretations (need neighbor register)
const has32bit = computed(() => neighborValues.value.length >= 2)

const longABCD = computed(() => {
  if (!has32bit.value) return '-'
  const [hi, lo] = neighborValues.value
  const val = ((hi << 16) | lo) >>> 0
  return val.toString()
})

const longCDAB = computed(() => {
  if (!has32bit.value) return '-'
  const [hi, lo] = neighborValues.value
  const val = ((lo << 16) | hi) >>> 0
  return val.toString()
})

function toFloat32(hi: number, lo: number): string {
  const buf = new ArrayBuffer(4)
  const view = new DataView(buf)
  view.setUint16(0, hi)
  view.setUint16(2, lo)
  return view.getFloat32(0).toPrecision(7)
}

const floatABCD = computed(() => {
  if (!has32bit.value) return '-'
  return toFloat32(neighborValues.value[0], neighborValues.value[1])
})

const floatCDAB = computed(() => {
  if (!has32bit.value) return '-'
  return toFloat32(neighborValues.value[1], neighborValues.value[0])
})

// 64-bit interpretation (need 4 consecutive registers)
const has64bit = computed(() => neighborValues.value.length >= 4)

const doubleValue = computed(() => {
  if (!has64bit.value) return '-'
  const buf = new ArrayBuffer(8)
  const view = new DataView(buf)
  for (let i = 0; i < 4; i++) {
    view.setUint16(i * 2, neighborValues.value[i])
  }
  return view.getFloat64(0).toPrecision(15)
})

// Load neighbor registers when selection changes
watch(selectedRegister, async (reg) => {
  if (!reg || !selectedConnectionId.value || selectedSlaveId.value === null) {
    neighborValues.value = []
    return
  }
  if (isBoolType.value) {
    neighborValues.value = []
    return
  }
  try {
    // Read 4 consecutive registers starting from selected address
    const values: number[] = []
    for (let i = 0; i < 4; i++) {
      const addr = reg.address + i
      if (addr > 65535) break
      const result = await invoke<{ address: number; value: number }>('read_register', {
        connectionId: selectedConnectionId.value,
        slaveId: selectedSlaveId.value,
        registerType: reg.register_type,
        address: addr,
      })
      values.push(result.value)
    }
    neighborValues.value = values
  } catch {
    neighborValues.value = [reg.value]
  }
}, { immediate: true })
</script>

<template>
  <div class="value-panel">
    <div class="panel-header">值解析</div>

    <div v-if="!selectedRegister" class="empty-state">
      选择一个寄存器查看详情
    </div>

    <template v-else>
      <div class="panel-title">{{ panelTitle }}</div>

      <!-- Bool type -->
      <template v-if="isBoolType">
        <div class="value-section">
          <div class="value-row">
            <span class="value-label">Value</span>
            <span class="value-data">{{ selectedRegister.value !== 0 ? 'true (1)' : 'false (0)' }}</span>
          </div>
          <div class="value-row">
            <span class="value-label">Hex</span>
            <span class="value-data mono">{{ hex16 }}</span>
          </div>
        </div>
      </template>

      <!-- 16-bit register -->
      <template v-else>
        <div class="value-section">
          <div class="section-title">16-bit</div>
          <div class="value-row">
            <span class="value-label">Signed</span>
            <span class="value-data mono">{{ signed16 }}</span>
          </div>
          <div class="value-row">
            <span class="value-label">Unsigned</span>
            <span class="value-data mono">{{ unsigned16 }}</span>
          </div>
          <div class="value-row">
            <span class="value-label">Hex</span>
            <span class="value-data mono">{{ hex16 }}</span>
          </div>
          <div class="value-row">
            <span class="value-label">Binary</span>
            <span class="value-data mono">{{ binary16 }}</span>
          </div>
        </div>

        <div v-if="has32bit" class="value-section">
          <div class="section-title">32-bit</div>
          <div class="value-row">
            <span class="value-label">Long AB CD</span>
            <span class="value-data mono">{{ longABCD }}</span>
          </div>
          <div class="value-row">
            <span class="value-label">Long CD AB</span>
            <span class="value-data mono">{{ longCDAB }}</span>
          </div>
          <div class="value-row">
            <span class="value-label">Float AB CD</span>
            <span class="value-data mono">{{ floatABCD }}</span>
          </div>
          <div class="value-row">
            <span class="value-label">Float CD AB</span>
            <span class="value-data mono">{{ floatCDAB }}</span>
          </div>
        </div>

        <div v-if="has64bit" class="value-section">
          <div class="section-title">64-bit</div>
          <div class="value-row">
            <span class="value-label">Double</span>
            <span class="value-data mono">{{ doubleValue }}</span>
          </div>
        </div>
      </template>
    </template>
  </div>
</template>

<style scoped>
.value-panel {
  padding: 0;
  font-size: 13px;
}

.panel-header {
  padding: 8px 12px;
  font-size: 11px;
  text-transform: uppercase;
  color: #6c7086;
  letter-spacing: 0.5px;
}

.empty-state {
  padding: 24px 12px;
  color: #6c7086;
  text-align: center;
  font-size: 12px;
}

.panel-title {
  padding: 6px 12px;
  font-size: 12px;
  font-weight: 600;
  color: #89b4fa;
  border-bottom: 1px solid #313244;
  margin-bottom: 4px;
}

.value-section {
  padding: 4px 0;
  border-bottom: 1px solid #313244;
}

.section-title {
  padding: 4px 12px;
  font-size: 11px;
  color: #6c7086;
  text-transform: uppercase;
}

.value-row {
  display: flex;
  justify-content: space-between;
  padding: 3px 12px;
}

.value-label {
  color: #6c7086;
  font-size: 12px;
}

.value-data {
  color: #cdd6f4;
  font-size: 12px;
  text-align: right;
}

.value-data.mono {
  font-family: 'SF Mono', 'Fira Code', monospace;
}
</style>
