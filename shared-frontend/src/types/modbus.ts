export interface LogEntry {
  timestamp: string
  direction: string
  function_code: string
  detail: string
}

export type DialogMode = 'alert' | 'confirm' | 'prompt'

export interface DialogState {
  visible: boolean
  mode: DialogMode
  title: string
  message: string
  defaultValue: string
  inputValue: string
}
