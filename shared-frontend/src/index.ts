// Types
export type { LogEntry, DialogMode, DialogState } from './types/modbus'

// Composables
export {
  showAlert,
  showConfirm,
  showPrompt,
  dialogConfirm,
  dialogCancel,
  useDialogState,
} from './composables/useDialog'

// Value formatting
export {
  swapBytes16,
  toFloat32,
  float32ToU16Pair,
  use16BitFormat,
  use32BitFormat,
  use64BitFormat,
} from './composables/useValueFormat'
export type { ByteOrder } from './composables/useValueFormat'

// Address formatting
export { formatAddress } from './composables/useAddressFormat'
export type { AddrMode } from './composables/useAddressFormat'

// FC / register-type label helpers
export { useFcLabel } from './composables/useFcLabel'

// Log panel
export { useLogPanel } from './composables/useLogPanel'
export type { LogPanelDataSource } from './composables/useLogPanel'
export { useLogFilter } from './composables/useLogFilter'
export type { DirectionFilter, FcFilter } from './composables/useLogFilter'

// Error handler
export { useErrorHandler } from './composables/useErrorHandler'
export type { Toast } from './composables/useErrorHandler'

// Components
export { default as AppDialog } from './components/AppDialog.vue'
export { default as LangToggle } from './components/LangToggle.vue'
export { default as LogPanelShell } from './components/LogPanelShell.vue'

// i18n
export { useI18n, type Locale } from './i18n'
