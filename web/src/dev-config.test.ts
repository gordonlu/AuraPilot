import { describe, expect, it } from 'vitest'
import tauriConfig from '../../src-tauri/tauri.conf.json'
import { DEFAULT_THEME, nextTheme, resolveTheme } from './theme'

describe('desktop development endpoint', () => {
  it('keeps the AuraPilot-associated, non-system port fixed in one source of truth', () => {
    const url = new URL(tauriConfig.build.devUrl)

    expect(url.hostname).toBe('localhost')
    expect(url.port).toBe('28727')
    expect(url.port).not.toContain('4')
  })
})

describe('theme defaults', () => {
  it('starts light while preserving an explicit saved choice', () => {
    expect(DEFAULT_THEME).toBe('light')
    expect(resolveTheme(null)).toBe('light')
    expect(resolveTheme('brand')).toBe('brand')
    expect(resolveTheme('dark')).toBe('dark')
    expect(nextTheme('light')).toBe('brand')
    expect(nextTheme('brand')).toBe('dark')
    expect(nextTheme('dark')).toBe('light')
  })
})
