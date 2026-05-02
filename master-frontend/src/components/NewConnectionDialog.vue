<script setup lang="ts">
import { ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { useI18n, showAlert } from 'shared-frontend'

const { t } = useI18n()

interface Props { show: boolean }
const props = defineProps<Props>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'created'): void
}>()

const form = ref({
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

watch(() => props.show, (visible) => {
  if (!visible) return
  form.value = { transport: 'tcp', target_address: '127.0.0.1', port: 502, slave_id: 1, timeout_ms: 3000 }
  serialPort.value = ''
  baudRate.value = 9600
  dataBits.value = 8
  stopBits.value = 1
  parityMode.value = 'none'
  useTls.value = false
  tlsCaFile.value = ''
  tlsCertFile.value = ''
  tlsKeyFile.value = ''
  tlsPkcs12File.value = ''
  tlsPkcs12Password.value = ''
  tlsAcceptInvalidCerts.value = false
})

watch(() => form.value.transport, (val) => {
  if (val === 'rtu' || val === 'ascii') refreshSerialPorts()
})

async function refreshSerialPorts() {
  try {
    serialPorts.value = await invoke('list_serial_ports')
  } catch (e) { await showAlert(String(e)) }
}

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
  } catch (e) { await showAlert(String(e)) }
}

async function submit() {
  const needsSerial = form.value.transport === 'rtu' || form.value.transport === 'ascii'
  if (needsSerial && !serialPort.value) {
    await showAlert(t('errors.serialPortRequired'))
    return
  }

  let transport: Record<string, unknown>
  if (form.value.transport === 'tcp') {
    transport = { type: 'tcp', host: form.value.target_address, port: form.value.port }
    if (useTls.value) transport = { type: 'tcp_tls', host: form.value.target_address, port: form.value.port }
  } else if (form.value.transport === 'rtu' || form.value.transport === 'ascii') {
    transport = {
      type: form.value.transport,
      serial_port: serialPort.value,
      baud_rate: baudRate.value,
      data_bits: dataBits.value,
      stop_bits: stopBits.value,
      parity: parityMode.value,
    }
  } else {
    transport = { type: 'rtu_over_tcp', host: form.value.target_address, port: form.value.port }
  }

  try {
    await invoke('create_master_connection', {
      request: {
        transport,
        slave_id: form.value.slave_id,
        timeout_ms: form.value.timeout_ms,
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
    emit('close')
    emit('created')
  } catch (e) { await showAlert(String(e)) }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="show" class="modal-backdrop" @click.self="emit('close')">
      <div class="modal-box">
        <div class="modal-title">{{ t('toolbar.newConnection') }}</div>
        <div class="modal-body">
          <label class="form-label">
            {{ t('dialog.transport') }}
            <select v-model="form.transport" class="form-input">
              <option value="tcp">TCP</option>
              <option value="rtu">{{ t('dialog.rtuSerial') }}</option>
              <option value="ascii">{{ t('dialog.asciiSerial') }}</option>
              <option value="rtu_over_tcp">RTU over TCP</option>
            </select>
          </label>
          <template v-if="form.transport === 'tcp' || form.transport === 'rtu_over_tcp'">
            <label class="form-label">
              {{ t('dialog.targetAddress') }}
              <input v-model="form.target_address" class="form-input" type="text" placeholder="127.0.0.1" />
            </label>
            <label class="form-label">
              {{ t('dialog.port') }}
              <input v-model.number="form.port" class="form-input" type="number" min="1" max="65535" />
            </label>
          </template>
          <template v-if="form.transport === 'tcp'">
            <label class="form-label">
              <input type="checkbox" v-model="useTls" /> {{ t('dialog.enableTls') }}
            </label>
            <template v-if="useTls">
              <label class="form-label">
                {{ t('dialog.caFile') }}
                <div class="file-row">
                  <input v-model="tlsCaFile" class="form-input" type="text" :placeholder="t('dialog.caFilePlaceholder')" />
                  <button class="tool-btn" @click="pickFile('ca')">...</button>
                </div>
              </label>
              <label class="form-label">
                {{ t('dialog.clientCert') }}
                <div class="file-row">
                  <input v-model="tlsCertFile" class="form-input" type="text" :placeholder="t('dialog.clientCertPlaceholder')" />
                  <button class="tool-btn" @click="pickFile('cert')">...</button>
                </div>
              </label>
              <label class="form-label">
                {{ t('dialog.clientKey') }}
                <div class="file-row">
                  <input v-model="tlsKeyFile" class="form-input" type="text" :placeholder="t('dialog.clientCertPlaceholder')" />
                  <button class="tool-btn" @click="pickFile('key')">...</button>
                </div>
              </label>
              <label class="form-label">
                {{ t('dialog.pkcs12File') }}
                <div class="file-row">
                  <input v-model="tlsPkcs12File" class="form-input" type="text" :placeholder="t('dialog.pkcs12FilePlaceholder')" />
                  <button class="tool-btn" @click="pickFile('pkcs12')">...</button>
                </div>
              </label>
              <label class="form-label" v-if="tlsPkcs12File">
                {{ t('dialog.pkcs12Password') }}
                <input v-model="tlsPkcs12Password" class="form-input" type="password" :placeholder="t('dialog.passwordPlaceholder')" />
              </label>
              <label class="form-label">
                <input type="checkbox" v-model="tlsAcceptInvalidCerts" /> {{ t('dialog.acceptInvalidCerts') }}
              </label>
            </template>
          </template>
          <template v-if="form.transport === 'rtu' || form.transport === 'ascii'">
            <label class="form-label">
              {{ t('dialog.serialPort') }}
              <div class="file-row">
                <select v-model="serialPort" class="form-input">
                  <option v-for="p in serialPorts" :key="p.name" :value="p.name">
                    {{ p.name }}{{ p.description ? ` (${p.description})` : '' }}
                  </option>
                </select>
                <button class="tool-btn" @click="refreshSerialPorts" :title="t('dialog.refreshSerialPorts')">&#x21bb;</button>
              </div>
            </label>
            <label class="form-label">
              {{ t('dialog.baudRate') }}
              <select v-model.number="baudRate" class="form-input">
                <option :value="9600">9600</option>
                <option :value="19200">19200</option>
                <option :value="38400">38400</option>
                <option :value="57600">57600</option>
                <option :value="115200">115200</option>
              </select>
            </label>
            <label class="form-label">
              {{ t('dialog.dataBits') }}
              <select v-model.number="dataBits" class="form-input">
                <option :value="7">7</option>
                <option :value="8">8</option>
              </select>
            </label>
            <label class="form-label">
              {{ t('dialog.stopBits') }}
              <select v-model.number="stopBits" class="form-input">
                <option :value="1">1</option>
                <option :value="2">2</option>
              </select>
            </label>
            <label class="form-label">
              {{ t('dialog.parity') }}
              <select v-model="parityMode" class="form-input">
                <option value="none">{{ t('dialog.parityNone') }}</option>
                <option value="odd">{{ t('dialog.parityOdd') }}</option>
                <option value="even">{{ t('dialog.parityEven') }}</option>
              </select>
            </label>
          </template>
          <label class="form-label">
            {{ t('dialog.slaveId') }}
            <input v-model.number="form.slave_id" class="form-input" type="number" min="1" max="247" />
          </label>
          <label class="form-label">
            {{ t('dialog.timeout') }}
            <input v-model.number="form.timeout_ms" class="form-input" type="number" min="100" max="30000" />
          </label>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="emit('close')">{{ t('common.cancel') }}</button>
          <button class="btn btn-primary" @click="submit">{{ t('common.create') }}</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-backdrop { position: fixed; inset: 0; background: rgba(0,0,0,0.5); display: flex; align-items: center; justify-content: center; z-index: 1000; }
.modal-box { background: #1e1e2e; border: 1px solid #45475a; border-radius: 8px; padding: 20px; min-width: 340px; box-shadow: 0 8px 24px rgba(0,0,0,0.5); }
.modal-title { font-size: 15px; font-weight: 600; color: #cdd6f4; margin-bottom: 16px; }
.modal-body { display: flex; flex-direction: column; gap: 12px; }
.modal-footer { display: flex; justify-content: flex-end; gap: 8px; margin-top: 20px; }
.form-label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: #6c7086; }
.form-input { padding: 6px 10px; background: #313244; border: 1px solid #45475a; border-radius: 4px; color: #cdd6f4; font-size: 13px; }
.form-input:focus { outline: none; border-color: #89b4fa; }
.file-row { display: flex; gap: 4px; }
.file-row > input, .file-row > select { flex: 1; }
.tool-btn { padding: 4px 8px; background: #313244; border: 1px solid #45475a; border-radius: 4px; color: #cdd6f4; cursor: pointer; font-size: 14px; }
.tool-btn:hover { background: #45475a; }
.btn { padding: 7px 20px; border: none; border-radius: 6px; cursor: pointer; font-size: 13px; }
.btn-primary { background: #89b4fa; color: #1e1e2e; }
.btn-primary:hover { background: #74c7ec; }
.btn-secondary { background: #45475a; color: #cdd6f4; }
.btn-secondary:hover { background: #585b70; }
</style>
