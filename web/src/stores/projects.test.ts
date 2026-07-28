import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

const { invoke, listen } = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}))
vi.mock('@tauri-apps/api/core', () => ({ invoke, isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen }))

import { PROJECT_CHANGED_EVENT, useProjectsStore } from './projects'

describe('projects store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    invoke.mockReset()
    listen.mockReset()
  })

  it('loads registry and snapshots from the backend', async () => {
    const project = { id: 'project-1', path: '/repo', registered_at: '2026-07-28T00:00:00Z' }
    const snapshot = { registration: project, project: null, tasks: [], diagnostics: [] }
    invoke.mockImplementation((command: string) =>
      Promise.resolve(command === 'list_projects' ? [project] : [snapshot]),
    )
    const store = useProjectsStore()
    await store.load()
    expect(store.projects).toEqual([project])
    expect(store.snapshots['project-1']).toEqual(snapshot)
  })

  it('refreshes only the project referenced by a watcher event', async () => {
    let handler: ((event: { payload: { project_id: string } }) => Promise<void>) | undefined
    listen.mockImplementation((_event: string, callback: typeof handler) => {
      handler = callback
      return Promise.resolve(() => undefined)
    })
    const snapshot = {
      registration: { id: 'project-1', path: '/repo', registered_at: '2026-07-28T00:00:00Z' },
      project: null,
      tasks: [],
      diagnostics: [],
    }
    invoke.mockResolvedValue(snapshot)
    const store = useProjectsStore()
    await store.startWatching()
    expect(listen).toHaveBeenCalledWith(PROJECT_CHANGED_EVENT, expect.any(Function))
    await handler?.({ payload: { project_id: 'project-1' } })
    expect(invoke).toHaveBeenCalledWith('scan_project', { id: 'project-1' })
    expect(store.snapshots['project-1']).toEqual(snapshot)
  })

  it('routes task CRUD through explicit Tauri commands and refreshes the project', async () => {
    const project = { id: 'project-1', path: '/repo', registered_at: '2026-07-28T00:00:00Z' }
    const task = { path: '/repo/.aurapilot/tasks/backlog/TASK-001.yaml', state: 'backlog', document: { id: 'TASK-001' } }
    const snapshot = { registration: project, project: null, tasks: [task], diagnostics: [] }
    invoke.mockImplementation((command: string) => Promise.resolve(command === 'scan_project' ? snapshot : task))
    const store = useProjectsStore()
    store.snapshots['project-1'] = snapshot as never
    const draft = { title: 'Task', priority: 'P1', task_type: 'feature', desc: null, accept: [] }

    await store.create('project-1', draft)
    await store.update('project-1', 'TASK-001', draft)
    await store.transition('project-1', 'TASK-001', { target: 'in-progress', assigned: 'codex', branch: 'task/one' })
    await store.deleteTask('project-1', 'TASK-001')

    expect(invoke).toHaveBeenCalledWith('create_task', { projectId: 'project-1', input: draft })
    expect(invoke).toHaveBeenCalledWith('update_task', { projectId: 'project-1', taskId: 'TASK-001', input: draft })
    expect(invoke).toHaveBeenCalledWith('transition_task', expect.objectContaining({ projectId: 'project-1', taskId: 'TASK-001' }))
    expect(invoke).toHaveBeenCalledWith('delete_task', { projectId: 'project-1', taskId: 'TASK-001' })
  })
})
