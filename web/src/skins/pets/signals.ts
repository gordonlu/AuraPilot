import type { ProjectSnapshot, TaskState } from '../../types/protocol'
import type { PetEventId } from './manifest'

export type BlockedTaskState = Map<string, boolean>

export const snapshotBlockedTasks = (
  snapshots: readonly ProjectSnapshot[],
): BlockedTaskState => new Map(snapshots.flatMap((snapshot) => snapshot.tasks.map((task) => [
  `${snapshot.registration.id}:${task.document.id ?? task.path}`,
  (task.document.blockers?.length ?? 0) > 0,
] as const)))

export const hasNewlyBlockedTask = (
  previous: BlockedTaskState,
  current: BlockedTaskState,
): boolean => [...current].some(([key, blocked]) => blocked && previous.get(key) !== true)

export const transitionPetEvent = (target: TaskState): PetEventId | null => {
  if (target === 'in-progress') return 'task-started'
  if (target === 'in-review') return 'task-review'
  if (target === 'done') return 'task-done'
  return null
}
