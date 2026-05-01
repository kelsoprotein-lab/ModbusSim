import { isLocale, type Locale } from './types'

export const STORAGE_KEY = 'modbussim.locale'

export function loadStoredLocale(): Locale | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return isLocale(raw) ? raw : null
  } catch {
    return null
  }
}

export function storeLocale(locale: Locale): void {
  try {
    localStorage.setItem(STORAGE_KEY, locale)
  } catch {
    // localStorage 被禁用时静默忽略
  }
}

export function detectFromNavigator(): Locale {
  const lang = (typeof navigator !== 'undefined' && navigator.language) || ''
  return lang.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en-US'
}

export function detectInitialLocale(): Locale {
  return loadStoredLocale() ?? detectFromNavigator()
}
