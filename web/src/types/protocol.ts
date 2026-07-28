export type Severity = 'info' | 'warning' | 'error' | 'blocked'
export type TaskState = 'backlog' | 'in-progress' | 'in-review' | 'done'

export interface Diagnostic {
  severity: Severity
  code: string
  message: string
  field: string | null
  path: string | null
}

export interface RegisteredProject {
  id: string
  path: string
  registered_at: string
  last_profile_id?: string | null
}

export interface ProjectDocument {
  name: string | null
  owner: string | null
  health: string | null
  sprint: string | null
  notes: string | null
  schema_version: number | null
  created: string | null
  [key: string]: unknown
}

export interface TaskDocument {
  id: string | null
  title: string | null
  priority: string | null
  type: string | null
  created: string | null
  assigned: string | null
  branch: string | null
  started: string | null
  pr: number | null
  waiting: string | null
  completed: string | null
  commit: string | null
  desc: string | null
  accept: string[]
  log: Array<Record<string, unknown>>
  blockers: string[]
  [key: string]: unknown
}

export interface LocatedTask {
  path: string
  state: TaskState
  document: TaskDocument
}

export interface ProjectSnapshot {
  registration: RegisteredProject
  project: ProjectDocument | null
  tasks: LocatedTask[]
  diagnostics: Diagnostic[]
}

export type ProjectChangeKind =
  | 'created'
  | 'modified'
  | 'removed'
  | 'renamed'
  | 'rescan_required'

export interface ProjectChange {
  project_id: string
  kind: ProjectChangeKind
  paths: string[]
}

export interface TaskDraft {
  title: string
  priority: string
  task_type: string
  desc: string | null
  accept: string[]
}

export interface TaskTransition {
  target: TaskState
  assigned?: string | null
  branch?: string | null
  pr?: number | null
  waiting?: string | null
  commit?: string | null
}

export type LaunchMode = 'interactive_terminal' | 'headless_process' | 'clipboard_only'
export type PromptTransport = 'argument' | 'stdin' | 'clipboard'

export type WorkingDirectory = { kind: 'repository' } | { kind: 'fixed_path'; path: string }

export interface AgentLaunchProfile {
  id: string
  display_name: string
  executable: string
  args: string[]
  working_directory: WorkingDirectory
  launch_mode: LaunchMode
  prompt_transport: PromptTransport
  detect_commands: string[]
  show_terminal: boolean
}

export interface ExecutableAvailability {
  available: boolean
  resolved_path: string | null
  detail: string
}

export interface AgentProfileEntry {
  profile: AgentLaunchProfile
  built_in: boolean
  availability: ExecutableAvailability
}

export interface PointerPrompt {
  task_id: string
  protocol_file: string
  task_file: string
  repository: string
  text: string
}

export type PushAttemptStatus = 'requested' | 'started' | 'failed_to_start' | 'exited'
export type PushDelivery = 'process' | 'clipboard' | 'clipboard_fallback'

export interface PushAttempt {
  id: string
  task_id: string
  project_id: string
  agent_profile_id: string
  created_at: string
  status: PushAttemptStatus
  process_id: number | null
  error: string | null
  delivery: PushDelivery
}

export interface PushOutcome {
  attempt: PushAttempt
  pointer_prompt: PointerPrompt
  message: string
}
