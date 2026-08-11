import { isTauri } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { defineStore } from 'pinia'
import { invokeBackend } from '../backend'
import type {
  AgentLaunchProfile,
  AgentProvider,
  AgentProfileEntry,
  AgentSessionBinding,
  ExecutionEvent,
  GitWorkspaceStatus,
  PointerPrompt,
  PushOutcome,
  PushAttempt,
} from '../types/protocol'

const demoProvider = (profileId: string): AgentProvider => {
  if (profileId === 'codex') return 'codex'
  if (profileId === 'claude-code') return 'claude_code'
  if (profileId === 'opencode') return 'open_code'
  return 'other'
}

export const PUSH_ATTEMPT_EVENT = 'aurapilot://push-attempt'
export const EXECUTION_EVENT = 'aurapilot://execution-event'

const builtin = (
  id: string,
  display_name: string,
  executable: string,
  args: string[],
): AgentProfileEntry => ({
  built_in: true,
  availability: { available: true, resolved_path: `/usr/local/bin/${executable}`, detail: '演示模式' },
  profile: {
    id, display_name, executable, args,
    working_directory: { kind: 'repository' },
    launch_mode: 'interactive_terminal',
    prompt_transport: 'argument',
    detect_commands: [executable],
    show_terminal: true,
  },
})

const demoProfiles = (): AgentProfileEntry[] => [
  builtin('codex', 'Codex', 'codex', ['{prompt}']),
  builtin('claude-code', 'Claude Code', 'claude', ['{prompt}']),
  builtin('gemini-cli', 'Gemini CLI', 'gemini', ['-i', '{prompt}']),
  builtin('cursor-agent', 'Cursor Agent CLI', 'cursor-agent', ['{prompt}']),
  builtin('opencode', 'OpenCode', 'opencode', ['--prompt', '{prompt}']),
  {
    built_in: true,
    availability: { available: true, resolved_path: null, detail: '通用兜底' },
    profile: {
      id: 'clipboard-only', display_name: '复制任务指令', executable: '', args: [],
      working_directory: { kind: 'repository' }, launch_mode: 'clipboard_only', prompt_transport: 'clipboard',
      detect_commands: [], show_terminal: false,
    },
  },
]

export const useAgentsStore = defineStore('agents', {
  state: () => ({
    profiles: [] as AgentProfileEntry[],
    loading: false,
    error: null as string | null,
    sessions: [] as AgentSessionBinding[],
    gitWorkspaces: {} as Record<string, GitWorkspaceStatus>,
    runtimeError: null as string | null,
    stopListening: null as UnlistenFn | null,
    stopExecutionListening: null as UnlistenFn | null,
    pushAttempts: [] as PushAttempt[],
    executionEvents: [] as ExecutionEvent[],
    executionLoading: false,
    executionError: null as string | null,
  }),
  actions: {
    async startWatchingAttempts() {
      if (!isTauri() || this.stopListening) return
      try {
        this.pushAttempts = await invokeBackend<PushAttempt[]>('list_push_attempts')
        this.executionEvents = await invokeBackend<ExecutionEvent[]>('list_execution_events', {
          projectId: null, taskId: null, limit: 300,
        })
      } catch (error) {
        this.executionError = `无法读取历史执行记录：${String(error)}`
      }
      this.stopListening = await listen<PushAttempt>(PUSH_ATTEMPT_EVENT, ({ payload }) => {
        const index = this.pushAttempts.findIndex((attempt) => attempt.id === payload.id)
        if (index >= 0) this.pushAttempts[index] = payload
        else this.pushAttempts.unshift(payload)
        if (!payload.error) return
        this.runtimeError = `${payload.task_id} · ${payload.agent_profile_id}：${payload.error}`
      })
      this.stopExecutionListening = await listen<ExecutionEvent>(EXECUTION_EVENT, ({ payload }) => {
        if (this.executionEvents.some((event) => event.id === payload.id)) return
        this.executionEvents.unshift(payload)
        if (this.executionEvents.length > 300) this.executionEvents.length = 300
        if (payload.level === 'error') {
          this.runtimeError = `${payload.task_id} · ${payload.profile_id}：${payload.message}${payload.detail ? `；${payload.detail}` : ''}`
        }
      })
    },
    stopWatchingAttempts() {
      this.stopListening?.()
      this.stopExecutionListening?.()
      this.stopListening = null
      this.stopExecutionListening = null
    },
    clearRuntimeError() {
      this.runtimeError = null
    },
    async loadExecutionEvents(projectId?: string, taskId?: string) {
      if (!isTauri()) return this.executionEvents
      this.executionLoading = true
      this.executionError = null
      try {
        this.executionEvents = await invokeBackend<ExecutionEvent[]>('list_execution_events', {
          projectId: projectId || null,
          taskId: taskId || null,
          limit: 300,
        })
        return this.executionEvents
      } catch (error) {
        this.executionError = String(error)
        throw error
      } finally {
        this.executionLoading = false
      }
    },
    async load() {
      this.loading = true
      this.error = null
      try {
        this.profiles = isTauri()
          ? await invokeBackend<AgentProfileEntry[]>('list_agent_profiles')
          : demoProfiles()
      } catch (error) {
        this.error = String(error)
      } finally {
        this.loading = false
      }
    },
    async preview(projectId: string, taskId: string): Promise<PointerPrompt> {
      if (isTauri()) return invokeBackend('preview_pointer_prompt', { projectId, taskId })
      return {
        task_id: taskId,
        protocol_file: '.aurapilot/AGENTS.md',
        task_file: `.aurapilot/tasks/backlog/${taskId}.yaml`,
        repository: '/demo/repository',
        text: `执行 AuraPilot 任务 ${taskId}。\n\n开始前必须读取：\n1. .aurapilot/AGENTS.md\n2. .aurapilot/tasks/backlog/${taskId}.yaml\n\n任务文件和协议文件是唯一事实来源。\n请按协议领取任务、执行、验证并更新进度。`,
      }
    },
    async loadSessions(projectId: string) {
      this.sessions = isTauri()
        ? await invokeBackend<AgentSessionBinding[]>('list_agent_sessions', { projectId })
        : []
    },
    async bindSession(
      projectId: string,
      profileId: string,
      externalSessionId: string,
      displayName?: string,
    ) {
      const session = isTauri()
        ? await invokeBackend<AgentSessionBinding>('bind_agent_session', {
          projectId, profileId, externalSessionId, displayName: displayName || null,
        })
        : {
          id: crypto.randomUUID(), project_id: projectId, profile_id: profileId,
          provider: demoProvider(profileId),
          external_session_id: externalSessionId, source: 'manual' as const,
          verification: 'unverified' as const, display_name: displayName || null,
          working_directory: '/demo/repository', state: 'not_loaded' as const,
          active_turn_id: null, hidden: false, created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(), last_used_at: new Date().toISOString(),
        }
      const index = this.sessions.findIndex((item) => item.id === session.id)
      if (index >= 0) this.sessions[index] = session
      else this.sessions.unshift(session)
      return session
    },
    async updateSession(
      projectId: string,
      sessionId: string,
      externalSessionId: string,
      displayName: string,
      confirmReplacement: boolean,
    ) {
      const current = this.sessions.find((item) => item.id === sessionId)
      if (!current) throw new Error(`Session binding not found: ${sessionId}`)
      const session = isTauri()
        ? await invokeBackend<AgentSessionBinding>('update_agent_session', {
          projectId, sessionId, externalSessionId,
          displayName: displayName.trim() || null,
          confirmReplacement,
        })
        : {
          ...current,
          external_session_id: externalSessionId.trim(),
          display_name: displayName.trim() || null,
          verification: 'unverified' as const,
          updated_at: new Date().toISOString(),
        }
      const index = this.sessions.findIndex((item) => item.id === session.id)
      if (index >= 0) this.sessions[index] = session
      return session
    },
    async gitStatus(projectId: string): Promise<GitWorkspaceStatus> {
      if (isTauri()) {
        const status = await invokeBackend<GitWorkspaceStatus>('get_git_workspace_status', { projectId })
        this.gitWorkspaces[projectId] = status
        return status
      }
      const status = this.gitWorkspaces[projectId] ?? {
        is_repository: true,
        current_branch: 'main',
        dirty: false,
        detail: 'Git 工作区干净（演示）',
      }
      this.gitWorkspaces[projectId] = status
      return status
    },
    async push(
      projectId: string,
      taskId: string,
      profileId: string,
      gitBranchName: string | null = null,
    ): Promise<PushOutcome> {
      if (isTauri()) return invokeBackend('push_task', { projectId, taskId, profileId, gitBranchName })
      const pointer_prompt = await this.preview(projectId, taskId)
      if (gitBranchName) {
        this.gitWorkspaces[projectId] = {
          is_repository: true,
          current_branch: gitBranchName,
          dirty: false,
          detail: 'Git 工作区干净（演示）',
        }
      }
      return {
        pointer_prompt,
        message: `${gitBranchName ? `Git 分支 ${gitBranchName} 已创建；` : ''}${this.profiles.find((item) => item.profile.id === profileId)?.profile.display_name} 已启动（演示）`,
        attempt: {
          id: crypto.randomUUID(), task_id: taskId, project_id: projectId,
          agent_profile_id: profileId, created_at: new Date().toISOString(), status: 'started',
          process_id: 1000, error: null, delivery: profileId === 'clipboard-only' ? 'clipboard' : 'process',
        },
      }
    },
    async pushExisting(projectId: string, taskId: string, sessionId: string): Promise<PushOutcome> {
      if (isTauri()) return invokeBackend('push_task_to_session', { projectId, taskId, sessionId })
      const pointer_prompt = await this.preview(projectId, taskId)
      const session = this.sessions.find((item) => item.id === sessionId) ?? null
      return {
        pointer_prompt, session,
        message: '已追加到现有 Session（演示）',
        attempt: {
          id: crypto.randomUUID(), task_id: taskId, project_id: projectId,
          agent_profile_id: session?.profile_id ?? 'codex', created_at: new Date().toISOString(),
          status: 'started', process_id: 1000, error: null, delivery: 'process',
        },
      }
    },
    async forkExisting(projectId: string, taskId: string, sessionId: string): Promise<PushOutcome> {
      if (isTauri()) return invokeBackend('fork_task_session', { projectId, taskId, sessionId })
      const pointer_prompt = await this.preview(projectId, taskId)
      const source = this.sessions.find((item) => item.id === sessionId)
      const session = source ? {
        ...source,
        id: crypto.randomUUID(),
        external_session_id: `thr_fork_${crypto.randomUUID().slice(0, 8)}`,
        display_name: `${source.display_name || source.profile_id} 分支`,
        state: 'running' as const,
        active_turn_id: 'turn_demo',
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
        last_used_at: new Date().toISOString(),
      } : null
      return {
        pointer_prompt, session,
        message: `已创建 ${source?.provider === 'open_code' ? 'OpenCode' : 'Codex'} Session 分支并接收任务（演示）`,
        attempt: {
          id: crypto.randomUUID(), task_id: taskId, project_id: projectId,
          agent_profile_id: source?.profile_id ?? 'codex', created_at: new Date().toISOString(),
          status: 'started', process_id: 1000, error: null, delivery: 'process',
        },
      }
    },
    async steerExisting(projectId: string, taskId: string, sessionId: string): Promise<PushOutcome> {
      if (isTauri()) return invokeBackend('steer_task_session', { projectId, taskId, sessionId })
      const pointer_prompt = await this.preview(projectId, taskId)
      const session = this.sessions.find((item) => item.id === sessionId) ?? null
      return {
        pointer_prompt, session, message: '已追加到 Codex 当前 Turn（演示）',
        attempt: {
          id: crypto.randomUUID(), task_id: taskId, project_id: projectId,
          agent_profile_id: session?.profile_id ?? 'codex', created_at: new Date().toISOString(),
          status: 'started', process_id: null, error: null, delivery: 'process',
        },
      }
    },
    async interruptExisting(projectId: string, taskId: string, sessionId: string): Promise<PushOutcome> {
      if (isTauri()) return invokeBackend('interrupt_task_session', { projectId, taskId, sessionId })
      const pointer_prompt = await this.preview(projectId, taskId)
      const source = this.sessions.find((item) => item.id === sessionId) ?? null
      const session = source ? { ...source, state: 'interrupting' as const } : null
      return {
        pointer_prompt, session,
        message: session?.provider === 'open_code'
          ? '已请求中断 OpenCode Session；将在确认空闲后按 FIFO 追加（演示）'
          : '已请求中断；将在 turn/completed 后按 FIFO 追加到原 Session（演示）',
        attempt: {
          id: crypto.randomUUID(), task_id: taskId, project_id: projectId,
          agent_profile_id: session?.profile_id ?? 'codex', created_at: new Date().toISOString(),
          status: 'requested', process_id: null, error: null, delivery: 'process',
        },
      }
    },
    async save(profile: AgentLaunchProfile) {
      if (isTauri()) {
        const entry = await invokeBackend<AgentProfileEntry>('save_agent_profile', { profile })
        const index = this.profiles.findIndex((item) => item.profile.id === entry.profile.id)
        if (index >= 0) this.profiles[index] = entry
        else this.profiles.push(entry)
      }
      else {
        const index = this.profiles.findIndex((item) => item.profile.id === profile.id)
        const entry = { profile, built_in: false, availability: { available: true, resolved_path: profile.executable, detail: '演示模式' } }
        if (index >= 0) this.profiles[index] = entry
        else this.profiles.push(entry)
      }
    },
    async remove(id: string) {
      if (isTauri()) {
        await invokeBackend('delete_agent_profile', { id })
        this.profiles = this.profiles.filter((item) => item.profile.id !== id)
      }
      else this.profiles = this.profiles.filter((item) => item.profile.id !== id)
    },
    async test(projectId: string, profileId: string) {
      if (!isTauri()) return { message: '只读测试已启动（演示）' }
      return invokeBackend<{ message: string }>('test_agent_profile', { projectId, profileId })
    },
  },
})
