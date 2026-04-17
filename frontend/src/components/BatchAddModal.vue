<script setup lang="ts">
import { ref, computed, inject } from 'vue'
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

interface Props {
  show: boolean
  existingRegisters: Register[]
  connectionId: string
  slaveId: number
}

const props = defineProps<Props>()
const emit = defineEmits<{
  close: []
  saved: []
}>()

const startAddress = ref<number>(0)
const endAddress = ref<number>(100)
const formType = ref('holding_register')
const formDataType = ref('uint16')
const formEndian = ref('big')
const namePrefix = ref('')
const isSaving = ref(false)

const count = computed(() => {
  const s = startAddress.value ?? 0
  const e = endAddress.value ?? 0
  return e >= s ? e - s + 1 : 0
})

const existingCount = computed(() => {
  const s = startAddress.value ?? 0
  const e = endAddress.value ?? 0
  const t = formType.value
  return props.existingRegisters.filter(
    (r) => r.register_type === t && r.address >= s && r.address <= e
  ).length
})

const newCount = computed(() => count.value - existingCount.value)

const isValid = computed(() => {
  return count.value > 0 && count.value <= 50000
})

async function handleConfirm() {
  if (!isValid.value) return
  isSaving.value = true

  const s = startAddress.value ?? 0
  const e = endAddress.value ?? 0
  const t = formType.value
  const existingSet = new Set(
    props.existingRegisters
      .filter((r) => r.register_type === t)
      .map((r) => r.address)
  )

  const registers = []
  for (let addr = s; addr <= e; addr++) {
    if (existingSet.has(addr)) continue
    registers.push({
      address: addr,
      register_type: formType.value,
      data_type: formDataType.value,
      endian: formEndian.value,
      name: namePrefix.value ? `${namePrefix.value}_${addr}` : '',
      comment: '',
    })
  }

  try {
    await invoke('import_registers', {
      request: {
        connection_id: props.connectionId,
        slave_id: props.slaveId,
        registers,
      },
    })
    emit('saved')
    emit('close')
  } catch (err) {
    await showAlert(`批量添加失败：${err}`)
  } finally {
    isSaving.value = false
  }
}

function handleBackdropClick(e: MouseEvent) {
  if ((e.target as HTMLElement).classList.contains('modal-backdrop')) {
    emit('close')
  }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="show" class="modal-backdrop" @click="handleBackdropClick">
      <div class="modal">
        <div class="modal-header">
          <span class="modal-title">批量添加寄存器</span>
          <button class="btn-close" @click="$emit('close')">×</button>
        </div>

        <div class="modal-body">
          <div class="form-row">
            <div class="form-group half">
              <label class="form-label">起始地址</label>
              <input v-model.number="startAddress" type="number" class="form-input" min="0" max="65535" />
            </div>
            <div class="form-group half">
              <label class="form-label">结束地址</label>
              <input v-model.number="endAddress" type="number" class="form-input" min="0" max="65535" />
            </div>
          </div>

          <div class="form-group">
            <label class="form-label">类型</label>
            <select v-model="formType" class="form-select">
              <option value="coil">线圈 (Coil)</option>
              <option value="discrete_input">离散输入 (Discrete Input)</option>
              <option value="input_register">输入寄存器 (Input Register)</option>
              <option value="holding_register">保持寄存器 (Holding Register)</option>
            </select>
          </div>

          <div class="form-group">
            <label class="form-label">数据类型</label>
            <select v-model="formDataType" class="form-select">
              <option value="bool">Bool</option>
              <option value="uint16">UInt16</option>
              <option value="int16">Int16</option>
              <option value="uint32">UInt32</option>
              <option value="int32">Int32</option>
              <option value="float32">Float32</option>
            </select>
          </div>

          <div class="form-group">
            <label class="form-label">字节序</label>
            <select v-model="formEndian" class="form-select">
              <option value="big">大端序 (Big Endian)</option>
              <option value="little">小端序 (Little Endian)</option>
              <option value="mid_big">中大端序 (Mid-Big)</option>
              <option value="mid_little">中小端序 (Mid-Little)</option>
            </select>
          </div>

          <div class="form-group">
            <label class="form-label">名称前缀（可选）</label>
            <input v-model="namePrefix" type="text" class="form-input" placeholder="如 HR → HR_0, HR_1, ..." />
          </div>

          <div class="count-info">
            <span v-if="count > 50000" class="count-warn">范围过大（最多 50000）</span>
            <template v-else>
              <span>共 {{ count }} 个地址</span>
              <span v-if="existingCount > 0" class="count-skip">，跳过 {{ existingCount }} 个已存在</span>
              <span>，将添加 <strong>{{ newCount }}</strong> 个寄存器</span>
            </template>
          </div>
        </div>

        <div class="modal-footer">
          <button class="btn btn-secondary" @click="$emit('close')" :disabled="isSaving">取消</button>
          <button class="btn btn-primary" @click="handleConfirm" :disabled="!isValid || isSaving">
            {{ isSaving ? '添加中...' : '确认' }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal {
  background: #1e1e2e;
  border: 1px solid #45475a;
  border-radius: 8px;
  width: 420px;
  max-width: 90vw;
  max-height: 90vh;
  overflow-y: auto;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid #313244;
}

.modal-title {
  font-size: 16px;
  font-weight: 600;
  color: #cdd6f4;
}

.btn-close {
  background: none;
  border: none;
  color: #6c7086;
  font-size: 20px;
  cursor: pointer;
  padding: 0 4px;
  line-height: 1;
}

.btn-close:hover {
  color: #cdd6f4;
}

.modal-body {
  padding: 20px;
}

.form-row {
  display: flex;
  gap: 12px;
}

.form-group {
  margin-bottom: 16px;
}

.form-group.half {
  flex: 1;
}

.form-label {
  display: block;
  font-size: 13px;
  color: #6c7086;
  margin-bottom: 6px;
}

.form-input,
.form-select {
  width: 100%;
  padding: 8px 12px;
  background: #11111b;
  border: 1px solid #45475a;
  border-radius: 6px;
  color: #cdd6f4;
  font-size: 14px;
  box-sizing: border-box;
}

.form-input:focus,
.form-select:focus {
  outline: none;
  border-color: #89b4fa;
}

.count-info {
  font-size: 13px;
  color: #a6adc8;
  padding: 8px 0;
}

.count-info strong {
  color: #a6e3a1;
}

.count-skip {
  color: #fab387;
}

.count-warn {
  color: #f38ba8;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 16px 20px;
  border-top: 1px solid #313244;
}

.btn {
  padding: 8px 20px;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
}

.btn-primary {
  background: #89b4fa;
  color: #1e1e2e;
}

.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-secondary {
  background: #45475a;
  color: #cdd6f4;
}

.btn-secondary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
