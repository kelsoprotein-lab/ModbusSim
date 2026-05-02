import { ref, readonly } from 'vue'
import type { DialogMode, DialogState } from '../types/modbus'
import { useI18n } from '../i18n'

const { t } = useI18n()

const state = ref<DialogState>({
  visible: false,
  mode: 'alert',
  title: '',
  message: '',
  defaultValue: '',
  inputValue: '',
})

let resolvePromise: ((value: string | boolean | null | undefined) => void) | null = null

function defaultTitle(mode: DialogMode): string {
  if (mode === 'alert') return t('dialog.alertTitle')
  if (mode === 'confirm') return t('dialog.confirmTitle')
  return t('dialog.promptTitle')
}

function cancelPending(mode: DialogMode) {
  if (!resolvePromise) return
  if (mode === 'alert') resolvePromise(undefined)
  else if (mode === 'confirm') resolvePromise(false)
  else resolvePromise(null)
  resolvePromise = null
}

interface OpenOptions {
  defaultValue?: string
  title?: string
}

function open(mode: DialogMode, message: string, opts: OpenOptions = {}): Promise<string | boolean | null | undefined> {
  // Discard any unresolved previous dialog so its caller doesn't hang forever.
  cancelPending(state.value.mode)
  return new Promise((resolve) => {
    resolvePromise = resolve
    const defaultValue = opts.defaultValue ?? ''
    state.value = {
      visible: true,
      mode,
      title: opts.title ?? defaultTitle(mode),
      message,
      defaultValue,
      inputValue: defaultValue,
    }
  })
}

export function showAlert(message: string, title?: string): Promise<void> {
  return open('alert', message, { title }) as Promise<void>
}

export function showConfirm(message: string, title?: string): Promise<boolean> {
  return open('confirm', message, { title }) as Promise<boolean>
}

export function showPrompt(message: string, defaultValue = '', title?: string): Promise<string | null> {
  return open('prompt', message, { defaultValue, title }) as Promise<string | null>
}

export function dialogConfirm(value?: string) {
  if (!resolvePromise) return
  const mode = state.value.mode
  state.value.visible = false
  if (mode === 'alert') resolvePromise(undefined)
  else if (mode === 'confirm') resolvePromise(true)
  else resolvePromise(value ?? state.value.inputValue)
  resolvePromise = null
}

export function dialogCancel() {
  if (!resolvePromise) return
  const mode = state.value.mode
  state.value.visible = false
  if (mode === 'alert') resolvePromise(undefined)
  else if (mode === 'confirm') resolvePromise(false)
  else resolvePromise(null)
  resolvePromise = null
}

export function useDialogState() {
  return { state: readonly(state), dialogConfirm, dialogCancel }
}
