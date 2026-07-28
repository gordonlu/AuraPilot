import { invoke, isTauri } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { defineStore } from 'pinia'
import { demoSnapshots } from '../demo'
import type {
  LocatedTask,
  ProjectChange,
  ProjectSnapshot,
  RegisteredProject,
  TaskDraft,
  TaskTransition,
} from '../types/protocol'

export const PROJECT_CHANGED_EVENT = 'aurapilot://project-changed'

export const useProjectsStore = defineStore('projects', {
  state: () => ({
    projects: [] as RegisteredProject[],
    snapshots: {} as Record<string, ProjectSnapshot>,
    loading: false,
    error: null as string | null,
    stopListening: null as UnlistenFn | null,
  }),
  actions: {
    async load() {
      if (!isTauri()) {
        if (import.meta.env.DEV && new URLSearchParams(window.location.search).get('empty') !== '1') {
          const snapshots = demoSnapshots()
          this.projects = snapshots.map((snapshot) => snapshot.registration)
          this.snapshots = Object.fromEntries(
            snapshots.map((snapshot) => [snapshot.registration.id, snapshot]),
          )
        }
        return
      }
      this.loading = true
      this.error = null
      try {
        const [projects, snapshots] = await Promise.all([
          invoke<RegisteredProject[]>('list_projects'),
          invoke<ProjectSnapshot[]>('scan_projects'),
        ])
        this.projects = projects
        this.snapshots = Object.fromEntries(
          snapshots.map((snapshot) => [snapshot.registration.id, snapshot]),
        )
      } catch (error) {
        this.error = String(error)
      } finally {
        this.loading = false
      }
    },
    async add(path: string) {
      if (!isTauri()) throw new Error('添加本地项目仅在桌面应用中可用')
      return this.register('add_project', path)
    },
    async initialize(path: string) {
      if (!isTauri()) throw new Error('初始化本地项目仅在桌面应用中可用')
      return this.register('initialize_project', path)
    },
    async register(command: 'add_project' | 'initialize_project', path: string) {
      const project = await invoke<RegisteredProject>(command, { path })
      this.projects.push(project)
      await this.refresh(project.id)
      return project
    },
    async chooseDirectory() {
      if (!isTauri()) throw new Error('目录选择仅在桌面应用中可用')
      const selected = await open({
        directory: true,
        multiple: false,
        title: '选择要接入 AuraPilot 的代码仓库',
      })
      return typeof selected === 'string' ? selected : null
    },
    async create(projectId: string, input: TaskDraft) {
      if (!isTauri()) {
        const snapshot = this.snapshots[projectId]
        if (!snapshot) throw new Error('项目不存在')
        const max = Object.values(this.snapshots)
          .flatMap((item) => item.tasks)
          .map((item) => Number(item.document.id?.replace('TASK-', '') ?? 0))
          .reduce((left, right) => Math.max(left, right), 0)
        const id = `TASK-${String(max + 1).padStart(3, '0')}`
        const task: LocatedTask = {
          path: `${snapshot.registration.path}/.aurapilot/tasks/backlog/${id}.yaml`,
          state: 'backlog',
          document: {
            id, title: input.title, priority: input.priority, type: input.task_type,
            created: new Date().toISOString().slice(0, 10), desc: input.desc, accept: input.accept,
            assigned: null, branch: null, started: null, pr: null, waiting: null,
            completed: null, commit: null, log: [], blockers: [],
          },
        }
        snapshot.tasks.push(task)
        return task
      }
      const task = await invoke<LocatedTask>('create_task', { projectId, input })
      await this.refresh(projectId)
      return task
    },
    async update(projectId: string, taskId: string, input: TaskDraft) {
      if (!isTauri()) {
        const task = this.snapshots[projectId]?.tasks.find((item) => item.document.id === taskId)
        if (!task) throw new Error('任务不存在')
        Object.assign(task.document, {
          title: input.title, priority: input.priority, type: input.task_type,
          desc: input.desc, accept: input.accept,
        })
        return task
      }
      const task = await invoke<LocatedTask>('update_task', { projectId, taskId, input })
      await this.refresh(projectId)
      return task
    },
    async transition(projectId: string, taskId: string, input: TaskTransition) {
      if (!isTauri()) {
        const task = this.snapshots[projectId]?.tasks.find((item) => item.document.id === taskId)
        if (!task) throw new Error('任务不存在')
        task.state = input.target
        task.document.assigned = input.target === 'backlog' ? null : (input.assigned ?? task.document.assigned)
        task.document.branch = input.target === 'backlog' ? null : (input.branch ?? task.document.branch)
        task.document.commit = input.target === 'done' ? (input.commit ?? task.document.commit) : null
        return task
      }
      const task = await invoke<LocatedTask>('transition_task', { projectId, taskId, input })
      await this.refresh(projectId)
      return task
    },
    async deleteTask(projectId: string, taskId: string) {
      if (!isTauri()) {
        const snapshot = this.snapshots[projectId]
        if (!snapshot) throw new Error('项目不存在')
        snapshot.tasks = snapshot.tasks.filter((item) => item.document.id !== taskId)
        return
      }
      await invoke('delete_task', { projectId, taskId })
      await this.refresh(projectId)
    },
    async remove(id: string) {
      await invoke<RegisteredProject>('remove_project', { id })
      this.projects = this.projects.filter((project) => project.id !== id)
      delete this.snapshots[id]
    },
    async refresh(id: string) {
      const snapshot = await invoke<ProjectSnapshot>('scan_project', { id })
      this.snapshots[id] = snapshot
    },
    async startWatching() {
      if (!isTauri() || this.stopListening) return
      this.stopListening = await listen<ProjectChange>(PROJECT_CHANGED_EVENT, async (event) => {
        try {
          await this.refresh(event.payload.project_id)
        } catch (error) {
          this.error = String(error)
        }
      })
    },
    stopWatching() {
      this.stopListening?.()
      this.stopListening = null
    },
  },
})
