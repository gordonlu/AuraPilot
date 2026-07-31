import { describe, expect, it } from 'vitest'
import {
  createHatchPetManifest,
  HATCH_PET_ANIMATIONS,
  HATCH_PET_ATLAS,
  HATCH_PET_EVENT_BINDINGS,
} from './hatchPet'

const hatchMetadata = {
  id: 'aura-hermit-crab',
  displayName: '星贝',
  description: '带着 Aura 星光贝壳在海岸陪伴任务推进的寄居蟹。',
  spritesheetPath: 'spritesheet.webp',
}

describe('Hatch Pet adapter', () => {
  it('fixes the official 8 × 9 atlas contract in one source of truth', () => {
    expect(HATCH_PET_ATLAS).toEqual({
      width: 1536,
      height: 1872,
      columns: 8,
      rows: 9,
      cellWidth: 192,
      cellHeight: 208,
    })
    expect(Object.keys(HATCH_PET_ANIMATIONS)).toEqual([
      'idle',
      'running-right',
      'running-left',
      'waving',
      'jumping',
      'failed',
      'waiting',
      'running',
      'review',
    ])
    expect(HATCH_PET_ANIMATIONS.idle.durationsMs).toEqual(
      [280, 110, 110, 140, 140, 320],
    )
    expect(HATCH_PET_ANIMATIONS.review).toEqual({
      row: 8,
      frames: [0, 1, 2, 3, 4, 5],
      durationsMs: [150, 150, 150, 150, 150, 280],
      loop: true,
    })
  })

  it('converts a Codex pet package into a validated AuraPilot manifest', () => {
    const manifest = createHatchPetManifest(hatchMetadata)

    expect(manifest.id).toBe('aura-hermit-crab')
    expect(manifest.atlas).toEqual({
      src: 'spritesheet.webp',
      ...HATCH_PET_ATLAS,
    })
    expect(manifest.animations).toEqual(HATCH_PET_ANIMATIONS)
    expect(manifest.eventBindings).toEqual(HATCH_PET_EVENT_BINDINGS)
    expect(manifest.interactions).toEqual([{
      id: 'pet-tap',
      label: '与星贝打招呼',
      bounds: { x: 0, y: 0, width: 1, height: 1 },
      animation: 'waving',
    }])
  })

  it('keeps unsafe or incomplete Hatch packages observable', () => {
    expect(() => createHatchPetManifest({
      ...hatchMetadata,
      spritesheetPath: '../outside.webp',
    })).toThrowError(/safe relative PNG or WebP path inside the pet package/)

    expect(() => createHatchPetManifest({
      ...hatchMetadata,
      id: undefined,
    })).toThrowError(/id must be a non-empty string/)
  })
})
