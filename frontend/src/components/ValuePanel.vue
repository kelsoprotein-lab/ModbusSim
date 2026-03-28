<script setup lang="ts">
import { inject, computed, ref, type Ref } from 'vue'

interface SelectedReg {
  address: number
  register_type: string
  value: number
}

const selectedRegister = inject<Ref<SelectedReg[]>>('selectedRegister')!
const addrMode = inject<Ref<'hex' | 'dec'>>('addrMode', ref('hex') as any)

const hasSelection = computed(() => selectedRegister.value.length > 0)

// Sort selected registers by address
const sortedRegs = computed(() => {
  return [...selectedRegister.value].sort((a, b) => a.address - b.address)
})

const firstReg = computed(() => sortedRegs.value[0] ?? null)

const isBoolType = computed(() => {
  if (!firstReg.value) return false
  return firstReg.value.register_type === 'coil' || firstReg.value.register_type === 'discrete_input'
})

// All selected regs are same type?
const allSameType = computed(() => {
  if (sortedRegs.value.length <= 1) return true
  const t = sortedRegs.value[0].register_type
  return sortedRegs.value.every(r => r.register_type === t)
})

const selCount = computed(() => sortedRegs.value.length)

function fmtAddress(addr: number): string {
  if (addrMode.value === 'dec') return addr.toString()
  return '0x' + addr.toString(16).toUpperCase().padStart(4, '0')
}

// Panel title
const panelTitle = computed(() => {
  if (!firstReg.value) return ''
  const prefixMap: Record<string, string> = {
    coil: '0x Coil',
    discrete_input: '1x Discrete Input',
    input_register: '3x Input Register',
    holding_register: '4x Holding Register',
  }
  const prefix = prefixMap[firstReg.value.register_type] || firstReg.value.register_type
  const fmtAddr = (addr: number) => addrMode.value === 'dec' ? addr.toString() : '0x' + addr.toString(16).toUpperCase().padStart(4, '0')

  if (selCount.value === 1) {
    return `${prefix} @ ${fmtAddr(firstReg.value.address)}`
  }
  const last = sortedRegs.value[sortedRegs.value.length - 1]
  return `${prefix} @ ${fmtAddr(firstReg.value.address)}~${fmtAddr(last.address)}`
})

// 16-bit interpretations (first selected register)
const signed16 = computed(() => {
  if (!firstReg.value) return 0
  const v = firstReg.value.value & 0xFFFF
  return v >= 0x8000 ? v - 0x10000 : v
})

const unsigned16 = computed(() => {
  if (!firstReg.value) return 0
  return firstReg.value.value & 0xFFFF
})

const hex16 = computed(() => {
  if (!firstReg.value) return '0x0000'
  return '0x' + (firstReg.value.value & 0xFFFF).toString(16).toUpperCase().padStart(4, '0')
})

const binary16 = computed(() => {
  if (!firstReg.value) return '0000 0000 0000 0000'
  const b = (firstReg.value.value & 0xFFFF).toString(2).padStart(16, '0')
  return `${b.slice(0, 4)} ${b.slice(4, 8)} ${b.slice(8, 12)} ${b.slice(12, 16)}`
})

// 32-bit: available when 2+ same-type non-bool regs selected
const show32bit = computed(() => {
  return selCount.value >= 2 && allSameType.value && !isBoolType.value
})

const reg32Values = computed(() => {
  if (!show32bit.value) return [0, 0]
  return [sortedRegs.value[0].value & 0xFFFF, sortedRegs.value[1].value & 0xFFFF]
})

const longABCD = computed(() => {
  if (!show32bit.value) return '-'
  const [hi, lo] = reg32Values.value
  return (((hi << 16) | lo) >>> 0).toString()
})

const longCDAB = computed(() => {
  if (!show32bit.value) return '-'
  const [hi, lo] = reg32Values.value
  return (((lo << 16) | hi) >>> 0).toString()
})

function toFloat32(hi: number, lo: number): string {
  const buf = new ArrayBuffer(4)
  const view = new DataView(buf)
  view.setUint16(0, hi)
  view.setUint16(2, lo)
  return view.getFloat32(0).toPrecision(7)
}

const floatABCD = computed(() => {
  if (!show32bit.value) return '-'
  return toFloat32(reg32Values.value[0], reg32Values.value[1])
})

const floatCDAB = computed(() => {
  if (!show32bit.value) return '-'
  return toFloat32(reg32Values.value[1], reg32Values.value[0])
})

// 64-bit: available when 4+ same-type non-bool regs selected
const show64bit = computed(() => {
  return selCount.value >= 4 && allSameType.value && !isBoolType.value
})

const doubleValue = computed(() => {
  if (!show64bit.value) return '-'
  const buf = new ArrayBuffer(8)
  const view = new DataView(buf)
  for (let i = 0; i < 4; i++) {
    view.setUint16(i * 2, sortedRegs.value[i].value & 0xFFFF)
  }
  return view.getFloat64(0).toPrecision(15)
})
</script>

<template>
  <div class="value-panel">
    <div class="panel-header">值解析</div>

    <div v-if="!hasSelection" class="empty-state">
      选择一个寄存器查看详情
    </div>

    <template v-else>
      <div class="panel-title">{{ panelTitle }}</div>
      <div v-if="selCount > 1 && !allSameType" class="panel-hint">
        选中了不同类型的寄存器，无法组合解析
      </div>

      <!-- Bool type -->
      <template v-if="isBoolType">
        <div class="value-section">
          <div v-for="reg in sortedRegs" :key="`${reg.register_type}-${reg.address}`" class="value-row">
            <span class="value-label">{{ fmtAddress(reg.address) }}</span>
            <span class="value-data">{{ reg.value !== 0 ? 'true (1)' : 'false (0)' }}</span>
          </div>
        </div>
      </template>

      <!-- Numeric register(s) -->
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

        <div v-if="show32bit" class="value-section">
          <div class="section-title">32-bit ({{ selCount >= 2 ? sortedRegs.slice(0, 2).map(r => fmtAddress(r.address)).join(' + ') : '' }})</div>
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

        <div v-if="show64bit" class="value-section">
          <div class="section-title">64-bit ({{ sortedRegs.slice(0, 4).map(r => fmtAddress(r.address)).join(' + ') }})</div>
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

.panel-hint {
  padding: 6px 12px;
  font-size: 11px;
  color: #fab387;
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
