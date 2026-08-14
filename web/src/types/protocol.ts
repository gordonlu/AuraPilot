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

export type RepairKind = 'fill_protocol_fields' | 'rename_file' | 'manual'
export type RepairAction =
  | { type: 'rewrite'; new_content: string; changes: string[] }
  | { type: 'rename_file'; target: string; new_content: string; changes: string[] }
  | { type: 'manual'; reason: string; suggestion: string }

export interface RepairPlan {
  id: string
  kind: RepairKind
  path: string
  summary: string
  detail: string
  action: RepairAction
  source_sha256: string
}

export interface AppliedRepair {
  kind: RepairKind
  path: string
  message: string
}

export interface RepairApplyReport {
  applied: AppliedRepair
  snapshot: ProjectSnapshot
}

export type PendingKind = 'approval' | 'repair'
export interface PendingItem {
  project_id: string
  project_name: string
  kind: PendingKind
  task_id: string | null
  title: string
  detail: string | null
  path: string | null
  repair_kind: RepairKind | null
  approval_id: string | null
  created_at: string
}

export interface PendingTarget {
  view: 'execution' | 'diagnostics'
  project_id: string
  approval_id: string | null
  path: string | null
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

export interface AuraExportReport {
  output: string
  encrypted: boolean
  task_count: number
  package_sha256: string
}

export interface AuraImportItem {
  task_id: string
  state: TaskState
  relative_path: string
  conflict: boolean
}

export interface AuraImportPreview {
  format_version: number
  encrypted: boolean
  package_sha256: string
  items: AuraImportItem[]
  has_conflicts: boolean
}

export interface AuraImportReport {
  imported: string[]
  package_sha256: string
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

export type PushAttemptStatus = 'requested' | 'started' | 'failed_to_start' | 'exited' | 'status_unknown'
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
  session?: AgentSessionBinding | null
}

export type ExecutionEventKind =
  | 'lifecycle' | 'command' | 'file_change' | 'agent_message'
  | 'reasoning' | 'approval' | 'provider' | 'error' | string
export type ExecutionEventLevel = 'info' | 'success' | 'warning' | 'error' | string

export interface ExecutionEvent {
  id: string
  project_id: string
  task_id: string
  profile_id: string
  provider: AgentProvider
  session_binding_id: string | null
  attempt_id: string | null
  kind: ExecutionEventKind
  level: ExecutionEventLevel
  phase: string
  message: string
  detail: string | null
  created_at: string
}

export type ApprovalKind = 'command_execution' | 'file_change'
export type ApprovalStatus = 'pending' | 'submitting' | 'approved' | 'declined' | 'expired' | 'failed'
export type ApprovalDecision = 'accept' | 'decline'

export interface ApprovalRequest {
  id: string
  project_id: string
  task_id: string | null
  profile_id: string
  provider: AgentProvider
  session_binding_id: string
  attempt_id: string | null
  turn_id: string
  item_id: string
  provider_request_key: string
  kind: ApprovalKind
  command: string | null
  cwd: string | null
  reason: string | null
  status: ApprovalStatus
  decision: ApprovalDecision | null
  error: string | null
  created_at: string
  updated_at: string
  resolved_at: string | null
}

export interface GitWorkspaceStatus {
  is_repository: boolean
  current_branch: string | null
  dirty: boolean
  detail: string
}

export type AgentProvider = 'codex' | 'claude_code' | 'open_code' | 'other'
export type SessionBindingSource = 'managed' | 'discovered' | 'integration_reported' | 'manual'
export type SessionVerification = 'verified' | 'unverified' | 'unavailable'
export type SessionRuntimeState =
  | 'starting' | 'idle' | 'running' | 'waiting_approval'
  | 'interrupting' | 'not_loaded' | 'terminated' | 'failed'

export interface AgentSessionBinding {
  id: string
  project_id: string
  profile_id: string
  provider: AgentProvider
  external_session_id: string
  source: SessionBindingSource
  verification: SessionVerification
  display_name: string | null
  working_directory: string
  state: SessionRuntimeState
  active_turn_id: string | null
  hidden: boolean
  created_at: string
  updated_at: string
  last_used_at: string
}
