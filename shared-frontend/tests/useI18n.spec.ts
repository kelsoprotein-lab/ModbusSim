import { beforeEach, describe, expect, it, vi } from 'vitest'
import { STORAGE_KEY } from '../src/i18n/detect'

// Mock localStorage
const mockStorage: Record<string, string> = {}

const localStorageMock = {
  getItem: (key: string) => mockStorage[key] ?? null,
  setItem: (key: string, value: string) => {
    mockStorage[key] = value
  },
  removeItem: (key: string) => {
    delete mockStorage[key]
  },
  clear: () => {
    Object.keys(mockStorage).forEach(key => delete mockStorage[key])
  },
  key: (index: number) => Object.keys(mockStorage)[index] ?? null,
  length: Object.keys(mockStorage).length,
}

async function freshI18n() {
  vi.resetModules()
  global.localStorage = localStorageMock as any
  return await import('../src/i18n')
}

beforeEach(() => {
  localStorageMock.clear()
  vi.spyOn(navigator, 'language', 'get').mockReturnValue('en-US')
})

describe('useI18n.t', () => {
  it('returns the value for the active locale', async () => {
    const { useI18n } = await freshI18n()
    const { t, setLocale } = useI18n()
    setLocale('zh-CN')
    expect(t('common.confirm')).toBe('确认')
    setLocale('en-US')
    expect(t('common.confirm')).toBe('Confirm')
  })

  it('replaces {placeholders} with params', async () => {
    const { useI18n } = await freshI18n()
    const { t, setLocale } = useI18n()
    setLocale('zh-CN')
    expect(t('station.defaultName', { id: 7 })).toBe('从站 7')
    setLocale('en-US')
    expect(t('station.defaultName', { id: 7 })).toBe('Slave 7')
  })

  it('falls back to en-US when key missing in zh-CN', async () => {
    const { useI18n } = await freshI18n()
    const { t, setLocale } = useI18n()
    setLocale('zh-CN')
    expect(t('common.confirm')).toBe('确认')
  })

  it('returns the key string when missing in both locales', async () => {
    const { useI18n } = await freshI18n()
    const { t } = useI18n()
    expect(t('definitely.not.a.real.key' as never)).toBe('definitely.not.a.real.key')
  })
})

describe('useI18n.setLocale', () => {
  it('persists to localStorage', async () => {
    const { useI18n } = await freshI18n()
    const { setLocale } = useI18n()
    setLocale('en-US')
    expect(localStorageMock.getItem(STORAGE_KEY)).toBe('en-US')
  })

  it('updates the reactive locale ref', async () => {
    const { useI18n } = await freshI18n()
    const { locale, setLocale } = useI18n()
    setLocale('zh-CN')
    expect(locale.value).toBe('zh-CN')
  })
})

describe('initial locale', () => {
  it('reads from localStorage when present', async () => {
    localStorageMock.setItem(STORAGE_KEY, 'zh-CN')
    vi.spyOn(navigator, 'language', 'get').mockReturnValue('en-US')
    const { useI18n } = await freshI18n()
    expect(useI18n().locale.value).toBe('zh-CN')
  })

  it('reads from navigator.language otherwise', async () => {
    vi.spyOn(navigator, 'language', 'get').mockReturnValue('zh-Hans-CN')
    const { useI18n } = await freshI18n()
    expect(useI18n().locale.value).toBe('zh-CN')
  })
})
