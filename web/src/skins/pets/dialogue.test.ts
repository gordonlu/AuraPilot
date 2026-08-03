import { describe, expect, it } from 'vitest'
import { dialogueForPetEvent, interactionDialogue, PET_DIALOGUE_CONFIG } from './dialogue'

describe('seascape pet dialogue', () => {
  it('describes real workflow events without exposing protocol details', () => {
    expect(dialogueForPetEvent('task-blocked')).toContain('搁浅')
    expect(dialogueForPetEvent('push-failed')).toContain('安全重试')
    expect(dialogueForPetEvent('task-blocked', 1)).not.toBe(dialogueForPetEvent('task-blocked'))
  })

  it('cycles interaction lines', () => {
    expect(interactionDialogue(PET_DIALOGUE_CONFIG.interactionLines.length)).toBe(interactionDialogue(0))
  })

  it('describes live board counts when context is available', () => {
    const context = { projects: 3, backlog: 4, inProgress: 2, inReview: 1, done: 6, blocked: 1 }
    expect(interactionDialogue(0, context)).toBe('1 个任务在浅滩搁浅，先看看最具体的阻塞。')
    expect(interactionDialogue(1, context)).toBe('潮间带里有 2 个任务正在推进。')
  })

  it('uses navigation language in the stellar world', () => {
    expect(dialogueForPetEvent('task-started', 0, 'stellar')).toContain('推进器')
    expect(interactionDialogue(0, {
      projects: 2,
      backlog: 1,
      inProgress: 0,
      inReview: 0,
      done: 0,
      blocked: 1,
    }, 'stellar')).toContain('航路受阻')
  })
})
