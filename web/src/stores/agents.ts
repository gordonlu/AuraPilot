import { isTauri } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { defineStore } from 'pinia'
import { invokeBackend } from '../backend'
import type {
  AgentLaunchProfile,
  AgentProfileEntry,
  AgentSessionBinding,
  PointerPrompt,
  PushOutcome,
  PushAttempt,
} from '../types/protocol'

export const PUSH_ATTEMPT_EVENT = 'aurapilot://push-attempt'

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
    runtimeError: null as string | null,
    stopListening: null as UnlistenFn | null,
  }),
  actions: {
    async startWatchingAttempts() {
      if (!isTauri() || this.stopListening) return
      this.stopListening = await listen<PushAttempt>(PUSH_ATTEMPT_EVENT, ({ payload }) => {
        if (!payload.error) return
        this.runtimeError = `${payload.task_id} · ${payload.agent_profile_id}：${payload.error}`
      })
    },
    stopWatchingAttempts() {
      this.stopListening?.()
      this.stopListening = null
    },
    clearRuntimeError() {
      this.runtimeError = null
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
          provider: profileId === 'codex' ? 'codex' as const : 'other' as const,
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
    async push(projectId: string, taskId: string, profileId: string): Promise<PushOutcome> {
      if (isTauri()) return invokeBackend('push_task', { projectId, taskId, profileId })
      const pointer_prompt = await this.preview(projectId, taskId)
      return {
        pointer_prompt,
        message: `${this.profiles.find((item) => item.profile.id === profileId)?.profile.display_name} 已启动（演示）`,
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
        message: '已创建 Codex Session 分支并接收任务（演示）',
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
        message: '已请求中断；将在 turn/completed 后按 FIFO 追加到原 Session（演示）',
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
