import { describe, expect, it } from 'vitest'
import tauriConfig from '../../src-tauri/tauri.conf.json'
import {
  DEFAULT_WORLD_SKIN,
  nextWorldSkin,
  resolveWorldSkin,
  WORLD_SKIN_CONFIG,
  WORLD_SKIN_STORAGE_KEY,
} from './skins/worldSkin'
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

describe('world skin defaults', () => {
  it('keeps the current UI as default and the experimental runtime bounded', () => {
    expect(DEFAULT_WORLD_SKIN).toBe('classic')
    expect(WORLD_SKIN_STORAGE_KEY).toBe('aurapilot-world-skin')
    expect(WORLD_SKIN_CONFIG.loadTimeoutMs).toBe(8_000)
    expect(resolveWorldSkin(null)).toBe('classic')
    expect(resolveWorldSkin('unknown')).toBe('classic')
    expect(resolveWorldSkin('seascape')).toBe('seascape')
    expect(resolveWorldSkin('stellar')).toBe('stellar')
    expect(nextWorldSkin('classic')).toBe('seascape')
    expect(nextWorldSkin('seascape')).toBe('stellar')
    expect(nextWorldSkin('stellar')).toBe('classic')
  })
})
