import { isTauri } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import { defineStore } from 'pinia'
import { invokeBackend, invokeLongBackend, withBackendTimeout } from '../backend'
import { demoSnapshots } from '../demo'
import type {
  LocatedTask,
  AuraExportReport,
  AuraImportPreview,
  AuraImportReport,
  ProjectChange,
  ProjectSnapshot,
  RegisteredProject,
  TaskDraft,
  TaskTransition,
} from '../types/protocol'

export const PROJECT_CHANGED_EVENT = 'aurapilot://project-changed'

const replaceTask = (snapshot: ProjectSnapshot | undefined, task: LocatedTask) => {
  if (!snapshot) return
  const index = snapshot.tasks.findIndex((item) => item.document.id === task.document.id)
  if (index === -1) snapshot.tasks.push(task)
  else snapshot.tasks[index] = task
}

const normalizeTask = (task: LocatedTask): LocatedTask => ({
  ...task,
  document: {
    ...task.document,
    accept: task.document.accept ?? [],
    log: task.document.log ?? [],
    blockers: task.document.blockers ?? [],
  },
})

const normalizeSnapshot = (snapshot: ProjectSnapshot): ProjectSnapshot => ({
  ...snapshot,
  tasks: snapshot.tasks.map(normalizeTask),
})

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
        const projects = await invokeBackend<RegisteredProject[]>('list_projects')
        this.projects = projects
        this.snapshots = Object.fromEntries(projects.map((project) => [
          project.id,
          this.snapshots[project.id] ?? {
            registration: project,
            project: null,
            tasks: [],
            diagnostics: [],
          },
        ]))
        for (const project of projects) this.refreshInBackground(project.id)
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
      const project = await invokeBackend<RegisteredProject>(command, { path })
      this.projects.push(project)
      this.snapshots[project.id] = {
        registration: project,
        project: null,
        tasks: [],
        diagnostics: [],
      }
      this.refreshInBackground(project.id)
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
    async chooseAuraExportPath(defaultName: string) {
      if (!isTauri()) throw new Error('导出仅在桌面应用中可用')
      return save({
        title: '导出 AuraPilot 任务包',
        defaultPath: defaultName,
        filters: [{ name: 'AuraPilot 任务包', extensions: ['aura'] }],
      })
    },
    async chooseAuraPackage() {
      if (!isTauri()) throw new Error('导入仅在桌面应用中可用')
      const selected = await open({
        title: '选择 AuraPilot 任务包',
        directory: false,
        multiple: false,
        filters: [{ name: 'AuraPilot 任务包', extensions: ['aura'] }],
      })
      return typeof selected === 'string' ? selected : null
    },
    async exportAura(projectId: string, taskIds: string[], output: string, password: string | null) {
      if (!isTauri()) throw new Error('导出仅在桌面应用中可用')
      return invokeLongBackend<AuraExportReport>('export_aura_tasks', {
        projectId, taskIds, output, password,
      })
    },
    async previewAuraImport(projectId: string, packagePath: string, password: string | null) {
      if (!isTauri()) throw new Error('导入仅在桌面应用中可用')
      return invokeLongBackend<AuraImportPreview>('preview_aura_import', {
        projectId, package: packagePath, password,
      })
    },
    async importAura(
      projectId: string,
      packagePath: string,
      password: string | null,
      expectedPackageSha256: string,
    ) {
      if (!isTauri()) throw new Error('导入仅在桌面应用中可用')
      const report = await invokeLongBackend<AuraImportReport>('import_aura_tasks', {
        projectId,
        package: packagePath,
        password,
        expectedPackageSha256,
      })
      await this.refresh(projectId)
      return report
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
      const task = normalizeTask(await invokeBackend<LocatedTask>('create_task', { projectId, input }))
      replaceTask(this.snapshots[projectId], task)
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
      const task = normalizeTask(await invokeBackend<LocatedTask>('update_task', { projectId, taskId, input }))
      replaceTask(this.snapshots[projectId], task)
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
      const task = normalizeTask(await invokeBackend<LocatedTask>('transition_task', { projectId, taskId, input }))
      replaceTask(this.snapshots[projectId], task)
      return task
    },
    async deleteTask(projectId: string, taskId: string) {
      if (!isTauri()) {
        const snapshot = this.snapshots[projectId]
        if (!snapshot) throw new Error('项目不存在')
        snapshot.tasks = snapshot.tasks.filter((item) => item.document.id !== taskId)
        return
      }
      await invokeBackend('delete_task', { projectId, taskId })
      const snapshot = this.snapshots[projectId]
      if (snapshot) snapshot.tasks = snapshot.tasks.filter((item) => item.document.id !== taskId)
    },
    async remove(id: string) {
      await invokeBackend<RegisteredProject>('remove_project', { id })
      this.projects = this.projects.filter((project) => project.id !== id)
      delete this.snapshots[id]
    },
    async refresh(id: string) {
      const snapshot = await invokeBackend<ProjectSnapshot>('scan_project', { id })
      this.snapshots[id] = normalizeSnapshot(snapshot)
    },
    refreshInBackground(id: string) {
      void this.refresh(id).catch((error) => {
        this.error = `项目后台刷新失败：${String(error)}`
      })
    },
    async startWatching() {
      if (!isTauri() || this.stopListening) return
      try {
        this.stopListening = await withBackendTimeout(
          listen<ProjectChange>(PROJECT_CHANGED_EVENT, (event) => {
            this.refreshInBackground(event.payload.project_id)
          }),
          'listen_project_changes',
        )
      } catch (error) {
        this.error = `无法启动项目监听：${String(error)}`
      }
    },
    stopWatching() {
      this.stopListening?.()
      this.stopListening = null
    },
  },
})
