import { describe, expect, it } from 'vitest'
import { remapPatrolPosition } from './actor'

describe('pet patrol placement', () => {
  it('keeps the pet at the right edge when a window grows', () => {
    expect(remapPatrolPosition(966, 620, 966, 1_050, 1_566)).toBe(1_566)
  })

  it('preserves patrol progress across a viewport resize', () => {
    expect(remapPatrolPosition(500, 200, 800, 400, 1_000)).toBe(700)
  })
})
