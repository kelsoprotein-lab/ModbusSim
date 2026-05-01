import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  STORAGE_KEY,
  detectInitialLocale,
  loadStoredLocale,
  storeLocale,
} from '../src/i18n/detect'

// Mock localStorage since jsdom's implementation is incomplete
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

beforeEach(() => {
  global.localStorage = localStorageMock as any
})

describe('detect.loadStoredLocale', () => {
  beforeEach(() => {
    localStorageMock.clear()
  })

  it('returns null when nothing stored', () => {
    expect(loadStoredLocale()).toBeNull()
  })

  it('returns the stored locale when valid', () => {
    localStorageMock.setItem(STORAGE_KEY, 'en-US')
    expect(loadStoredLocale()).toBe('en-US')
  })

  it('returns null when stored value is invalid', () => {
    localStorageMock.setItem(STORAGE_KEY, 'fr-FR')
    expect(loadStoredLocale()).toBeNull()
  })
})

describe('detect.storeLocale', () => {
  afterEach(() => {
    localStorageMock.clear()
  })

  it('writes the locale into localStorage', () => {
    storeLocale('zh-CN')
    expect(localStorageMock.getItem(STORAGE_KEY)).toBe('zh-CN')
  })
})

describe('detect.detectInitialLocale', () => {
  beforeEach(() => {
    localStorageMock.clear()
  })

  it('prefers stored locale over navigator.language', () => {
    localStorageMock.setItem(STORAGE_KEY, 'en-US')
    vi.spyOn(navigator, 'language', 'get').mockReturnValue('zh-CN')
    expect(detectInitialLocale()).toBe('en-US')
  })

  it('falls back to zh-CN when navigator.language starts with "zh"', () => {
    vi.spyOn(navigator, 'language', 'get').mockReturnValue('zh-Hans-CN')
    expect(detectInitialLocale()).toBe('zh-CN')
  })

  it('falls back to en-US otherwise', () => {
    vi.spyOn(navigator, 'language', 'get').mockReturnValue('ja-JP')
    expect(detectInitialLocale()).toBe('en-US')
  })

  it('falls back to en-US when navigator.language is empty', () => {
    vi.spyOn(navigator, 'language', 'get').mockReturnValue('')
    expect(detectInitialLocale()).toBe('en-US')
  })
})
