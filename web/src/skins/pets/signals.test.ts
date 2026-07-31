import { describe, expect, it } from 'vitest'
import { demoSnapshots } from '../../demo'
import {
  hasNewlyBlockedTask,
  snapshotBlockedTasks,
  transitionPetEvent,
} from './signals'

describe('pet signals from real task state', () => {
  it('only reports a task when it changes from unblocked to blocked', () => {
    const snapshots = demoSnapshots()
    for (const snapshot of snapshots) {
      for (const task of snapshot.tasks) task.document.blockers = []
    }
    const baseline = snapshotBlockedTasks(snapshots)

    snapshots[0].tasks[0].document.blockers = ['waiting for product decision']
    const blocked = snapshotBlockedTasks(snapshots)

    expect(hasNewlyBlockedTask(baseline, blocked)).toBe(true)
    expect(hasNewlyBlockedTask(blocked, blocked)).toBe(false)
  })

  it('treats a newly observed blocked task as a new blocking event after baseline', () => {
    const snapshots = demoSnapshots()
    const current = snapshotBlockedTasks(snapshots)

    expect(hasNewlyBlockedTask(new Map(), current)).toBe(
      snapshots.some((snapshot) => snapshot.tasks.some((task) => task.document.blockers.length > 0)),
    )
  })

  it('maps only meaningful workflow transitions to pet states', () => {
    expect(transitionPetEvent('backlog')).toBeNull()
    expect(transitionPetEvent('in-progress')).toBe('task-started')
    expect(transitionPetEvent('in-review')).toBe('task-review')
    expect(transitionPetEvent('done')).toBe('task-done')
  })
})
