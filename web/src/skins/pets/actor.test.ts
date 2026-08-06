import { describe, expect, it } from 'vitest'
import { POLARSCAPE_PET_CONFIG, remapPatrolPosition } from './actor'

describe('pet patrol placement', () => {
  it('keeps the pet at the right edge when a window grows', () => {
    expect(remapPatrolPosition(966, 620, 966, 1_050, 1_566)).toBe(1_566)
  })

  it('preserves patrol progress across a viewport resize', () => {
    expect(remapPatrolPosition(500, 200, 800, 400, 1_000)).toBe(700)
  })

  it('keeps the arctic fox inside the left snowfield patrol zone', () => {
    expect(POLARSCAPE_PET_CONFIG.initialEdge).toBe('left')
    expect(POLARSCAPE_PET_CONFIG.patrolLeftRatio).toBe(0.06)
    expect(POLARSCAPE_PET_CONFIG.patrolRightRatio).toBe(0.42)
    expect(POLARSCAPE_PET_CONFIG.packagePath).toBe('pets/aura-snowfox/')
    expect(POLARSCAPE_PET_CONFIG.interactionAnimation).toBe('waiting')
  })
})
