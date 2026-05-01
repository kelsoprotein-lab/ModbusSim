export type Locale = 'zh-CN' | 'en-US'

export const SUPPORTED_LOCALES: readonly Locale[] = ['zh-CN', 'en-US'] as const

export function isLocale(value: unknown): value is Locale {
  return typeof value === 'string' && (SUPPORTED_LOCALES as readonly string[]).includes(value)
}
