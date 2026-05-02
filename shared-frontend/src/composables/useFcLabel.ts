import { useI18n } from '../i18n'

const FC_KEYS: Record<string, string> = {
  read_coils: 'fc.fc01',
  read_discrete_inputs: 'fc.fc02',
  read_holding_registers: 'fc.fc03',
  read_input_registers: 'fc.fc04',
  write_single_coil: 'fc.fc05',
  write_single_register: 'fc.fc06',
  write_multiple_coils: 'fc.fc15',
  write_multiple_registers: 'fc.fc16',
}

const REG_TYPE_KEYS: Record<string, string> = {
  coil: 'fc.coil',
  discrete_input: 'fc.discreteInput',
  holding_register: 'fc.holdingRegister',
  input_register: 'fc.inputRegister',
}

export function useFcLabel() {
  const { t } = useI18n()

  function fcLabel(fn: string): string {
    const key = FC_KEYS[fn]
    return key ? t(key) : fn
  }

  function registerTypeLabel(type: string): string {
    const key = REG_TYPE_KEYS[type]
    return key ? t(key) : type
  }

  return { fcLabel, registerTypeLabel }
}
