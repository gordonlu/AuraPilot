import { isTauri } from '@tauri-apps/api/core'
import { defineStore } from 'pinia'
import { invokeBackend } from '../backend'
import type {
  AgentLaunchProfile,
  AgentProfileEntry,
  PointerPrompt,
  PushOutcome,
} from '../types/protocol'

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
  }),
  actions: {
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
