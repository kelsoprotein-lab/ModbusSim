export type AddrMode = 'hex' | 'dec'

export function formatAddress(addr: number, mode: AddrMode): string {
  if (mode === 'dec') return addr.toString()
  return '0x' + addr.toString(16).toUpperCase().padStart(4, '0')
}
