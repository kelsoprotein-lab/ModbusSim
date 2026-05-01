<script setup lang="ts">
import { ref, watch, computed, inject } from 'vue'
import { dialogKey } from '../composables/useDialog'
import type { showAlert as ShowAlert } from '../composables/useDialog'
import { useI18n } from 'shared-frontend'

const { t } = useI18n()
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
  mode: 'add' | 'edit'
  register?: Register
  existingRegisters: Register[]
  connectionId: string
  slaveId: number
}

const props = defineProps<Props>()
const emit = defineEmits<{
  close: []
  saved: []
}>()

// Form state
const formName = ref('')
const formAddress = ref<number | undefined>(undefined)
const formType = ref('holding_register')
const formDataType = ref('uint16')
const formEndian = ref('big')
const formComment = ref('')
const showAdvanced = ref(false)

// Conflict dialog
const showConflict = ref(false)

// Reset form when modal opens or mode/register changes
watch(
  () => [props.show, props.mode, props.register],
  () => {
    if (props.show) {
      if (props.mode === 'edit' && props.register) {
        formName.value = props.register.name || ''
        formAddress.value = props.register.address
        formType.value = props.register.register_type
        formDataType.value = props.register.data_type
        formEndian.value = props.register.endian
        formComment.value = props.register.comment || ''
      } else {
        formName.value = ''
        formAddress.value = undefined
        formType.value = 'holding_register'
        formDataType.value = 'uint16'
        formEndian.value = 'big'
        formComment.value = ''
      }
      showConflict.value = false
    }
  },
  { immediate: true }
)

const modalTitle = computed(() => (props.mode === 'add' ? t('registerEdit.addTitle') : t('registerEdit.editTitle')))

// Check for address conflict before saving
function hasConflict(): boolean {
  if (formAddress.value === undefined) return false
  return props.existingRegisters.some(
    (r) =>
      r.address === formAddress.value &&
      r.register_type === formType.value &&
      (props.mode === 'add' || r.address !== props.register?.address || r.register_type !== props.register?.register_type)
  )
}

async function handleConfirm() {
  if (formAddress.value === undefined) return

  if (hasConflict()) {
    showConflict.value = true
    return
  }

  await save()
}

async function handleOverride() {
  showConflict.value = false
  await save()
}

async function save() {
  const { invoke } = await import('@tauri-apps/api/core')
  try {
    if (props.mode === 'edit') {
      // Remove old register first
      if (props.register) {
        await invoke('remove_register', {
          connectionId: props.connectionId,
          slaveId: props.slaveId,
          address: props.register.address,
          registerType: props.register.register_type,
        })
      }
    }
    // Add new/updated register
    await invoke('add_register', {
      request: {
        connection_id: props.connectionId,
        slave_id: props.slaveId,
        address: formAddress.value,
        register_type: formType.value,
        data_type: formDataType.value,
        endian: formEndian.value,
        name: formName.value || null,
        comment: formComment.value || null,
      },
    })
    emit('saved')
    emit('close')
  } catch (e) {
    await showAlert(t('errors.operationFailed', { err: String(e) }))
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
        <!-- Header -->
        <div class="modal-header">
          <span class="modal-title">{{ modalTitle }}</span>
          <button class="btn-close" @click="$emit('close')">×</button>
        </div>

        <!-- Body -->
        <div class="modal-body">
          <!-- Basic fields -->
          <div class="form-group">
            <label class="form-label">{{ t('dialog.simpleName') }}</label>
            <input v-model="formName" type="text" class="form-input" :placeholder="t('registerEdit.namePlaceholder')" />
          </div>

          <div class="form-group">
            <label class="form-label">{{ t('table.address') }}</label>
            <input v-model.number="formAddress" type="number" class="form-input" min="0" max="65535" />
          </div>

          <div class="form-group">
            <label class="form-label">{{ t('table.type') }}</label>
            <select v-model="formType" class="form-select">
              <option value="coil">{{ t('table.coil') }} (Coil)</option>
              <option value="discrete_input">{{ t('table.discreteInput') }} (Discrete Input)</option>
              <option value="input_register">{{ t('table.inputRegister') }} (Input Register)</option>
              <option value="holding_register">{{ t('table.holdingRegister') }} (Holding Register)</option>
            </select>
          </div>

          <!-- Advanced options toggle -->
          <div class="form-group">
            <button class="btn-link" @click="showAdvanced = !showAdvanced">
              {{ showAdvanced ? `▼ ${t('registerEdit.hideAdvanced')}` : `▶ ${t('registerEdit.advanced')}` }}
            </button>
          </div>

          <!-- Advanced fields -->
          <template v-if="showAdvanced">
            <div class="form-group">
              <label class="form-label">{{ t('dialog.dataType') }}</label>
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
              <label class="form-label">{{ t('dialog.byteOrder') }}</label>
              <select v-model="formEndian" class="form-select">
                <option value="big">{{ t('dialog.byteOrderBig') }}</option>
                <option value="little">{{ t('dialog.byteOrderLittle') }}</option>
                <option value="mid_big">{{ t('dialog.byteOrderMidBig') }}</option>
                <option value="mid_little">{{ t('dialog.byteOrderMidLittle') }}</option>
              </select>
            </div>

            <div class="form-group">
              <label class="form-label">{{ t('registerEdit.commentLabel') }}</label>
              <input v-model="formComment" type="text" class="form-input" :placeholder="t('registerEdit.namePlaceholder')" />
            </div>
          </template>
        </div>

        <!-- Footer -->
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="$emit('close')">{{ t('common.cancel') }}</button>
          <button class="btn btn-primary" @click="handleConfirm">{{ t('common.confirm') }}</button>
        </div>

        <!-- Conflict dialog -->
        <div v-if="showConflict" class="conflict-dialog">
          <div class="conflict-title">{{ t('registerEdit.conflictTitle') }}</div>
          <div class="conflict-body">{{ t('registerEdit.conflictBody') }}</div>
          <div class="conflict-footer">
            <button class="btn btn-secondary" @click="showConflict = false">{{ t('common.cancel') }}</button>
            <button class="btn btn-danger" @click="handleOverride">{{ t('common.override') }}</button>
          </div>
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
  position: relative;
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

.form-group {
  margin-bottom: 16px;
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

.btn-link {
  background: none;
  border: none;
  color: #89b4fa;
  font-size: 13px;
  cursor: pointer;
  padding: 0;
}

.btn-link:hover {
  text-decoration: underline;
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

.btn-secondary {
  background: #45475a;
  color: #cdd6f4;
}

.btn-danger {
  background: #f38ba8;
  color: #1e1e2e;
}

/* Conflict dialog */
.conflict-dialog {
  position: absolute;
  inset: 0;
  background: rgba(30, 30, 46, 0.97);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
}

.conflict-title {
  font-size: 16px;
  font-weight: 600;
  color: #f38ba8;
  margin-bottom: 12px;
}

.conflict-body {
  font-size: 14px;
  color: #cdd6f4;
  margin-bottom: 24px;
}

.conflict-footer {
  display: flex;
  gap: 12px;
}
</style>
