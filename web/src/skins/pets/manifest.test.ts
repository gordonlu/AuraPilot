import { describe, expect, it } from 'vitest'
import {
  PET_MANIFEST_FORMAT_VERSION,
  PET_MANIFEST_LIMITS,
  PetManifestValidationError,
  parsePetManifest,
} from './manifest'

const validManifest = () => ({
  formatVersion: 1,
  id: 'orange-cat',
  displayName: '橘猫',
  description: '在海岸边陪伴任务推进的橘猫。',
  atlas: {
    src: 'spritesheet.webp',
    width: 1536,
    height: 1872,
    columns: 8,
    rows: 9,
    cellWidth: 192,
    cellHeight: 208,
  },
  animations: {
    idle: {
      row: 0,
      frames: [0, 1, 2, 3, 4, 5],
      durationsMs: [280, 110, 110, 140, 140, 320],
      loop: true,
    },
    'running-right': {
      row: 1,
      frames: [0, 1, 2, 3, 4, 5, 6, 7],
      durationsMs: [120, 120, 120, 120, 120, 120, 120, 220],
      loop: true,
    },
    greeting: {
      row: 3,
      frames: [0, 1, 2, 3],
      durationsMs: [140, 140, 140, 280],
      loop: false,
    },
    blocked: {
      row: 6,
      frames: [0, 1, 2, 3, 4, 5],
      durationsMs: [150, 150, 150, 150, 150, 260],
      loop: true,
    },
  },
  eventBindings: {
    'task-created': 'greeting',
    'task-blocked': 'blocked',
  },
  interactions: [{
    id: 'head',
    label: '摸摸橘猫',
    bounds: { x: 0.25, y: 0.05, width: 0.5, height: 0.45 },
    animation: 'greeting',
  }],
})

describe('AuraPilot pet manifest v1', () => {
  it('keeps the format and safety limits centralized', () => {
    expect(PET_MANIFEST_FORMAT_VERSION).toBe(1)
    expect(PET_MANIFEST_LIMITS).toEqual({
      maxAtlasDimension: 8192,
      maxCellDimension: 1024,
      maxAnimations: 64,
      maxFramesPerAnimation: 64,
      minFrameDurationMs: 16,
      maxFrameDurationMs: 60_000,
      maxInteractions: 32,
    })
  })

  it('accepts a Hatch Pet-compatible atlas with AuraPilot event bindings', () => {
    expect(parsePetManifest(validManifest())).toEqual(validManifest())
  })

  it('rejects paths that can escape the local pet package', () => {
    const manifest = validManifest()
    manifest.atlas.src = '../outside.webp'

    expect(() => parsePetManifest(manifest)).toThrowError(
      /safe relative PNG or WebP path inside the pet package/,
    )
  })

  it('rejects atlas geometry that does not match its grid', () => {
    const manifest = validManifest()
    manifest.atlas.width = 1500

    expect(() => parsePetManifest(manifest)).toThrowError(
      /atlas\.width must equal atlas\.columns × atlas\.cellWidth/,
    )
  })

  it('rejects animations outside the atlas and incomplete timings', () => {
    const manifest = validManifest()
    manifest.animations.idle.row = 9
    manifest.animations.idle.frames = [8]
    manifest.animations.idle.durationsMs = []

    expect(() => parsePetManifest(manifest)).toThrowError(PetManifestValidationError)
    expect(() => parsePetManifest(manifest)).toThrowError(/animations\.idle\.row/)
    expect(() => parsePetManifest(manifest)).toThrowError(/one duration for every frame/)
  })

  it('rejects missing animation references and out-of-frame hit areas', () => {
    const manifest = validManifest()
    manifest.eventBindings['task-created'] = 'missing'
    manifest.interactions[0].animation = 'missing'
    manifest.interactions[0].bounds.x = 0.8

    expect(() => parsePetManifest(manifest)).toThrowError(
      /references missing animation missing/,
    )
    expect(() => parsePetManifest(manifest)).toThrowError(
      /bounds must remain inside the normalized pet frame/,
    )
  })
})
