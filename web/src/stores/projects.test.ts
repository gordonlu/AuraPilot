import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

const { invoke, listen, open } = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  open: vi.fn(),
}))
vi.mock('@tauri-apps/api/core', () => ({ invoke, isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open }))

import { PROJECT_CHANGED_EVENT, useProjectsStore } from './projects'
import type { ProjectSnapshot } from '../types/protocol'

describe('projects store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    invoke.mockReset()
    listen.mockReset()
    open.mockReset()
  })

  it('selects a repository directory and can initialize it before registration', async () => {
    const project = { id: 'project-1', path: '/repo without protocol', registered_at: '2026-07-28T00:00:00Z' }
    const snapshot = { registration: project, project: null, tasks: [], diagnostics: [] }
    open.mockResolvedValue('/repo without protocol')
    invoke.mockImplementation((command: string) => Promise.resolve(
      command === 'initialize_project' ? project : snapshot,
    ))

    const store = useProjectsStore()
    expect(await store.chooseDirectory()).toBe('/repo without protocol')
    expect(open).toHaveBeenCalledWith(expect.objectContaining({ directory: true, multiple: false }))

    const registered = await store.initialize('/repo without protocol')
    expect(registered).toEqual(project)
    expect(invoke).toHaveBeenCalledWith('initialize_project', { path: '/repo without protocol' })
    expect(invoke).toHaveBeenCalledWith('scan_project', { id: 'project-1' })
    expect(store.projects).toEqual([project])
  })

  it('loads registry and snapshots from the backend', async () => {
    const project = { id: 'project-1', path: '/repo', registered_at: '2026-07-28T00:00:00Z' }
    const snapshot = { registration: project, project: null, tasks: [], diagnostics: [] }
    invoke.mockImplementation((command: string) =>
      Promise.resolve(command === 'list_projects' ? [project] : snapshot),
    )
    const store = useProjectsStore()
    await store.load()
    expect(store.projects).toEqual([project])
    expect(store.loading).toBe(false)
    await vi.waitFor(() => expect(store.snapshots['project-1']).toEqual(snapshot))
    expect(invoke).not.toHaveBeenCalledWith('scan_projects')
  })

  it('shows registered projects before a slow project scan finishes', async () => {
    const project = { id: 'project-1', path: '/slow-repo', registered_at: '2026-07-28T00:00:00Z' }
    let finishScan: ((snapshot: ProjectSnapshot) => void) | undefined
    const pendingScan = new Promise<ProjectSnapshot>((resolve) => { finishScan = resolve })
    invoke.mockImplementation((command: string) => command === 'list_projects'
      ? Promise.resolve([project])
      : pendingScan)
    const store = useProjectsStore()

    await store.load()

    expect(store.loading).toBe(false)
    expect(store.projects).toEqual([project])
    expect(store.snapshots['project-1']).toMatchObject({ registration: project, tasks: [] })
    finishScan?.({ registration: project, project: null, tasks: [], diagnostics: [] } as ProjectSnapshot)
    await pendingScan
  })

  it('normalizes protocol arrays omitted from compact backend JSON', async () => {
    const project = { id: 'project-1', path: '/repo', registered_at: '2026-07-28T00:00:00Z' }
    const compactSnapshot = {
      registration: project,
      project: null,
      tasks: [{
        path: '/repo/.aurapilot/tasks/backlog/TASK-001.yaml',
        state: 'backlog',
        document: { id: 'TASK-001', title: 'Compact task' },
      }],
      diagnostics: [],
    }
    invoke.mockImplementation((command: string) => Promise.resolve(
      command === 'list_projects' ? [project] : compactSnapshot,
    ))
    const store = useProjectsStore()

    await store.load()
    await vi.waitFor(() => expect(store.snapshots['project-1'].tasks).toHaveLength(1))

    expect(store.snapshots['project-1'].tasks[0].document).toMatchObject({
      accept: [], log: [], blockers: [],
    })
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
    await vi.waitFor(() => expect(store.snapshots['project-1']).toEqual(snapshot))
  })

  it('routes task CRUD through explicit Tauri commands and updates the local snapshot', async () => {
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

  it('finishes task creation without waiting for a follow-up project scan', async () => {
    const project = { id: 'project-1', path: '/repo', registered_at: '2026-07-28T00:00:00Z' }
    const task = {
      path: '/repo/.aurapilot/tasks/backlog/TASK-001.yaml',
      state: 'backlog',
      document: { id: 'TASK-001', title: 'Task' },
    }
    const snapshot = { registration: project, project: null, tasks: [], diagnostics: [] }
    invoke.mockImplementation((command: string) => {
      if (command === 'create_task') return Promise.resolve(task)
      if (command === 'scan_project') return new Promise(() => undefined)
      return Promise.resolve(undefined)
    })
    const store = useProjectsStore()
    store.snapshots['project-1'] = snapshot as never

    const created = await store.create('project-1', {
      title: 'Task', priority: 'P1', task_type: 'feature', desc: null, accept: [],
    })

    expect(created).toMatchObject({
      ...task,
      document: { ...task.document, accept: [], log: [], blockers: [] },
    })
    expect(store.snapshots['project-1'].tasks).toEqual([created])
    expect(invoke).not.toHaveBeenCalledWith('scan_project', expect.anything())
  })

  it('finishes project registration while its initial scan continues in the background', async () => {
    const project = { id: 'project-1', path: '/repo', registered_at: '2026-07-28T00:00:00Z' }
    const scanned = { registration: project, project: null, tasks: [], diagnostics: [] }
    let finishScan: ((snapshot: typeof scanned) => void) | undefined
    const pendingScan = new Promise<typeof scanned>((resolve) => { finishScan = resolve })
    invoke.mockImplementation((command: string) => command === 'add_project'
      ? Promise.resolve(project)
      : pendingScan)
    const store = useProjectsStore()

    await expect(store.add('/repo')).resolves.toEqual(project)

    expect(store.projects).toEqual([project])
    expect(store.snapshots['project-1']).toMatchObject({ registration: project, tasks: [] })
    expect(invoke).toHaveBeenCalledWith('scan_project', { id: 'project-1' })
    finishScan?.(scanned)
    await pendingScan
  })
})
