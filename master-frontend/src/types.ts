export interface MasterConnectionInfo {
  id: string
  target_address: string
  port: number
  slave_id: number
  state: string
  scan_group_count: number
}

export interface ScanGroupInfo {
  id: string
  name: string
  function: string
  start_address: number
  quantity: number
  interval_ms: number
  enabled: boolean
  is_polling: boolean
  slave_id: number | null
}

export interface RegisterValueDto {
  address: number
  raw_value: number
  display_value: string
  is_bool: boolean
}

export interface ReadResultDto {
  scan_group_id: string
  function: string
  start_address: number
  values: RegisterValueDto[]
  timestamp: string
  error: string | null
}

export interface PollDataPayload {
  connection_id: string
  scan_group_id: string
  result: ReadResultDto
}

export interface PollErrorPayload {
  connection_id: string
  scan_group_id: string
  error: string
}

export interface LogEntry {
  timestamp: string
  direction: string
  function_code: string
  detail: string
}

export interface SlaveIdScanEvent {
  connection_id: string
  current_id: number
  total: number
  found_ids: number[]
  done: boolean
  cancelled: boolean
}

export interface RegisterScanEvent {
  connection_id: string
  current_address: number
  end_address: number
  found_count: number
  found_registers: FoundRegisterDto[]
  done: boolean
  cancelled: boolean
}

export interface FoundRegisterDto {
  address: number
  value: number
}
