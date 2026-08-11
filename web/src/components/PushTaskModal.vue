<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useAgentsStore } from '../stores/agents'
import type { GitWorkspaceStatus, LocatedTask, PointerPrompt, ProjectSnapshot, PushOutcome } from '../types/protocol'
import type { PetEventId } from '../skins/pets/manifest'
import UiIcon from './UiIcon.vue'

const props = defineProps<{ task: LocatedTask; project: ProjectSnapshot }>()
const emit = defineEmits<{ close: []; petEvent: [event: PetEventId] }>()
const agents = useAgentsStore()
const mode = ref<'new' | 'existing'>('new')
const selectedProfile = ref('')
const selectedSession = ref('')
const preview = ref<PointerPrompt | null>(null)
const outcome = ref<PushOutcome | null>(null)
const busy = ref(false)
const error = ref<string | null>(null)
const showManual = ref(false)
const manualSessionId = ref('')
const manualName = ref('')
const showEdit = ref(false)
const editSessionId = ref('')
const editName = ref('')
const confirmReplacement = ref(false)
const branchStrategy = ref<'current' | 'new'>('current')
const branchName = ref(`task/${props.task.document.id ?? 'new-task'}`)
const gitStatus = ref<GitWorkspaceStatus | null>(null)
const gitError = ref<string | null>(null)
const branchResult = ref<string | null>(null)
const branchResultSuccess = ref(false)

const selectedEntry = computed(() => agents.profiles.find((entry) => entry.profile.id === selectedProfile.value))
const selectedBinding = computed(() => agents.sessions.find((session) => session.id === selectedSession.value))
const canForkSession = computed(() => ['codex', 'open_code'].includes(selectedBinding.value?.provider ?? '')
  && ['idle', 'not_loaded'].includes(selectedBinding.value?.state ?? ''))
const canSteerLiveTurn = computed(() => selectedBinding.value?.provider === 'codex'
  && selectedBinding.value.state === 'running' && Boolean(selectedBinding.value.active_turn_id))
const canInterruptSession = computed(() => selectedBinding.value?.state === 'running'
  && ((selectedBinding.value.provider === 'open_code' && Boolean(selectedBinding.value.active_turn_id))
    || (selectedBinding.value.provider === 'codex' && Boolean(selectedBinding.value.active_turn_id))))
const copiesOnly = computed(() => selectedEntry.value?.profile.launch_mode === 'clipboard_only')
const hasActiveProjectSession = computed(() => agents.sessions.some((session) => [
  'starting', 'running', 'waiting_approval', 'interrupting',
].includes(session.state)))
const selectedSessionIsActive = computed(() => selectedBinding.value ? [
  'starting', 'running', 'waiting_approval', 'interrupting',
].includes(selectedBinding.value.state) : false)
const editChangesManagedId = computed(() => Boolean(selectedBinding.value
  && selectedBinding.value.source !== 'manual'
  && editSessionId.value.trim() !== selectedBinding.value.external_session_id))
const primaryLabel = computed(() => {
  if (busy.value) return mode.value === 'existing' ? '正在启动后台 Run…' : copiesOnly.value ? '正在复制…' : '正在创建并绑定…'
  if (mode.value === 'existing') return '启动后台 Run'
  if (copiesOnly.value) return '复制任务指令'
  return `新建 Session 并 Push 给 ${selectedEntry.value?.profile.display_name ?? 'Agent'}`
})
const canSubmit = computed(() => {
  if (mode.value === 'existing') return Boolean(selectedBinding.value)
  if (!selectedEntry.value) return false
  return branchStrategy.value === 'current'
    || Boolean(gitStatus.value?.is_repository && branchName.value.trim() && !hasActiveProjectSession.value)
})
const shortId = (value: string) => value.length > 22 ? `${value.slice(0, 12)}…${value.slice(-7)}` : value
const formatTime = (value: string) => new Date(value).toLocaleString('zh-CN', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })

onMounted(async () => {
  try {
    const gitInspection = agents.gitStatus(props.project.registration.id)
      .then((status) => { gitStatus.value = status })
      .catch((caught) => { gitError.value = `无法读取 Git 状态：${String(caught)}` })
    await Promise.all([agents.load(), agents.loadSessions(props.project.registration.id), gitInspection])
    selectedProfile.value = agents.profiles.find((entry) => entry.profile.id === props.project.registration.last_profile_id)?.profile.id
      ?? agents.profiles.find((entry) => entry.availability.available && entry.profile.id !== 'clipboard-only')?.profile.id
      ?? agents.profiles.find((entry) => entry.profile.id === 'clipboard-only')?.profile.id
      ?? ''
    selectedSession.value = agents.sessions[0]?.id ?? ''
    if (props.task.document.id) preview.value = await agents.preview(props.project.registration.id, props.task.document.id)
  } catch (caught) {
    error.value = `无法准备 Push：${String(caught)}`
  }
})

const bindManual = async () => {
  if (!manualSessionId.value.trim() || !selectedProfile.value) return
  busy.value = true; error.value = null
  try {
    const session = await agents.bindSession(
      props.project.registration.id, selectedProfile.value,
      manualSessionId.value.trim(), manualName.value.trim(),
    )
    selectedSession.value = session.id
    mode.value = 'existing'; showManual.value = false
    manualSessionId.value = ''; manualName.value = ''
  } catch (caught) { error.value = `绑定 Session 失败：${String(caught)}` }
  finally { busy.value = false }
}

const beginEdit = () => {
  if (!selectedBinding.value || selectedSessionIsActive.value) return
  editSessionId.value = selectedBinding.value.external_session_id
  editName.value = selectedBinding.value.display_name ?? ''
  confirmReplacement.value = false
  showEdit.value = true
  showManual.value = false
}

const saveEdit = async () => {
  if (!selectedBinding.value || !editSessionId.value.trim() || selectedSessionIsActive.value) return
  busy.value = true; error.value = null
  try {
    const session = await agents.updateSession(
      props.project.registration.id,
      selectedBinding.value.id,
      editSessionId.value.trim(),
      editName.value,
      confirmReplacement.value,
    )
    selectedSession.value = session.id
    showEdit.value = false
  } catch (caught) { error.value = `更新 Session 失败：${String(caught)}` }
  finally { busy.value = false }
}

const applySessionOutcome = (result: PushOutcome) => {
  if (!result.session) return
  const index = agents.sessions.findIndex((item) => item.id === result.session?.id)
  if (index >= 0) agents.sessions[index] = result.session
  else agents.sessions.unshift(result.session)
  selectedSession.value = result.session.id
}

const push = async () => {
  if (!props.task.document.id || !canSubmit.value) return
  busy.value = true; error.value = null; outcome.value = null; branchResult.value = null; branchResultSuccess.value = false
  emit('petEvent', 'push-started')
  try {
    const requestedBranch = mode.value === 'new' && branchStrategy.value === 'new'
      ? branchName.value.trim()
      : null
    outcome.value = mode.value === 'existing'
      ? await agents.pushExisting(props.project.registration.id, props.task.document.id, selectedSession.value)
      : await agents.push(
        props.project.registration.id,
        props.task.document.id,
        selectedProfile.value,
        requestedBranch,
      )
    if (requestedBranch) {
      try {
        gitStatus.value = await agents.gitStatus(props.project.registration.id)
        branchResultSuccess.value = gitStatus.value.current_branch === requestedBranch
        branchResult.value = branchResultSuccess.value
          ? `当前工作树已切换到 ${requestedBranch}`
          : `分支操作后当前分支为 ${gitStatus.value.current_branch ?? 'detached HEAD'}，请检查后再继续`
        if (branchResultSuccess.value) branchStrategy.value = 'current'
      } catch (caught) {
        branchResultSuccess.value = false
        branchResult.value = `Push 已返回，但无法重新确认 Git 分支：${String(caught)}`
      }
    }
    if (mode.value === 'new') props.project.registration.last_profile_id = selectedProfile.value
    applySessionOutcome(outcome.value)
    emit('petEvent', outcome.value.attempt.status === 'failed_to_start'
      ? 'push-failed'
      : 'push-succeeded')
  } catch (caught) {
    error.value = String(caught)
    emit('petEvent', 'push-failed')
  }
  finally { busy.value = false }
}

const forkSession = async () => {
  if (!props.task.document.id || !selectedBinding.value || !canForkSession.value) return
  busy.value = true; error.value = null; outcome.value = null
  try {
    outcome.value = await agents.forkExisting(
      props.project.registration.id,
      props.task.document.id,
      selectedBinding.value.id,
    )
    applySessionOutcome(outcome.value)
  } catch (caught) { error.value = String(caught) }
  finally { busy.value = false }
}

const controlLiveTurn = async (action: 'steer' | 'interrupt') => {
  if (!props.task.document.id || !selectedBinding.value
    || (action === 'steer' ? !canSteerLiveTurn.value : !canInterruptSession.value)) return
  busy.value = true; error.value = null; outcome.value = null
  try {
    outcome.value = action === 'steer'
      ? await agents.steerExisting(props.project.registration.id, props.task.document.id, selectedBinding.value.id)
      : await agents.interruptExisting(props.project.registration.id, props.task.document.id, selectedBinding.value.id)
    applySessionOutcome(outcome.value)
  } catch (caught) { error.value = String(caught) }
  finally { busy.value = false }
}
</script>

<template>
  <div class="modal-backdrop" @mousedown.self="$emit('close')">
    <section class="task-modal push-modal" role="dialog" aria-modal="true" aria-label="Push 任务">
      <header>
        <div><span class="modal-mark"><UiIcon name="send"/></span><div><h2>Push {{ task.document.id }}</h2><p>选择打开新 CLI 或由 AuraPilot 在后台执行</p></div></div>
        <button class="icon-button" aria-label="关闭" @click="$emit('close')"><UiIcon name="x"/></button>
      </header>
      <div class="modal-body">
        <div class="push-mode-switch" role="tablist" aria-label="Session 选择">
          <button :class="{ active: mode === 'new' }" role="tab" :aria-selected="mode === 'new'" @click="mode = 'new'">新 Session</button>
          <button :class="{ active: mode === 'existing' }" role="tab" :aria-selected="mode === 'existing'" @click="mode = 'existing'">后台执行 <b>{{ agents.sessions.length }}</b></button>
        </div>

        <template v-if="mode === 'new'">
          <p class="push-notice"><strong>新建 Session 会打开一个新的 Agent CLI 窗口。</strong>Session 会锁定所选 Profile；“复制任务指令”不会创建 Run、Session 或 CLI 窗口。</p>
          <div v-if="agents.loading" class="inline-loading">正在检测 Agent…</div>
          <div v-else class="agent-grid">
            <button v-for="entry in agents.profiles" :key="entry.profile.id" :class="['agent-option', { selected: selectedProfile === entry.profile.id }]" @click="selectedProfile = entry.profile.id">
              <span :class="['availability-dot', { available: entry.availability.available }]"/>
              <strong>{{ entry.profile.display_name }}</strong>
              <small>{{ entry.availability.available ? entry.availability.detail : '未检测到，可使用剪贴板兜底' }}</small>
              <b>{{ entry.profile.launch_mode === 'clipboard_only' ? '仅复制' : '新建 Session' }}</b>
            </button>
          </div>
          <section class="git-branch-section" aria-labelledby="git-branch-title">
            <div><strong id="git-branch-title">Git 分支策略</strong><small>与 Agent Session 分支是两个独立概念</small></div>
            <label :class="['branch-option', { selected: branchStrategy === 'current' }]">
              <input v-model="branchStrategy" type="radio" value="current"/>
              <span><strong>沿用当前分支</strong><small>{{ gitStatus?.current_branch ?? (gitStatus?.is_repository ? 'detached HEAD' : '不会执行 Git 操作') }}</small></span>
            </label>
            <label :class="['branch-option', { selected: branchStrategy === 'new', disabled: !gitStatus?.is_repository || hasActiveProjectSession }]">
              <input v-model="branchStrategy" type="radio" value="new" :disabled="!gitStatus?.is_repository || hasActiveProjectSession"/>
              <span><strong>创建并切换到新 Git 分支</strong><small>分支创建成功后才会启动 Agent</small></span>
            </label>
            <label v-if="branchStrategy === 'new'" class="field branch-name"><span>新分支名称</span><input v-model="branchName" required autocomplete="off" placeholder="task/TASK-001"/></label>
            <p v-if="gitStatus?.dirty" class="branch-warning"><UiIcon name="alert" :size="14"/>工作区有未提交变更；创建分支时会保留这些变更。</p>
            <p v-if="hasActiveProjectSession" class="branch-warning"><UiIcon name="alert" :size="14"/>项目有正在工作的 Session，暂时不能切换工作树分支。</p>
            <p v-if="gitError" class="branch-warning"><UiIcon name="alert" :size="14"/>{{ gitError }}</p>
          </section>
        </template>

        <template v-else>
          <p class="push-notice"><strong>后台 Agent 不会出现在当前 Agent CLI，但可在 AuraPilot“执行中心”查看 Provider 回传的过程。</strong>它会通过独立连接恢复所选 Session 的已保存上下文，通常会产生额外 Token 消耗。AuraPilot 无法保证会话上下文仍适合当前任务；不同 Provider 可回传的细节不同，请检查 Profile、项目和 Session ID。此模式不会切换 Git 分支。</p>
          <div v-if="agents.sessions.length" class="session-list">
            <button v-for="session in agents.sessions" :key="session.id" :class="['session-option', { selected: selectedSession === session.id }]" @click="selectedSession = session.id">
              <span :class="['session-state', session.state]"/>
              <span><strong>{{ session.display_name || `${session.profile_id} Session` }}</strong><small>{{ session.profile_id }} · {{ session.provider }} · {{ formatTime(session.last_used_at) }}</small></span>
              <code>{{ shortId(session.external_session_id) }}</code>
              <b :class="session.verification">{{ session.verification === 'verified' ? '已验证' : session.verification === 'unavailable' ? '不可用' : '未验证' }}</b>
            </button>
          </div>
          <div v-else class="session-empty"><strong>这个项目还没有已记录的 Session</strong><span>可以先新建 Session，或手动绑定已有 ID。</span></div>
          <div v-if="selectedBinding && ['codex', 'open_code'].includes(selectedBinding.provider)" class="session-actions">
            <button v-if="canSteerLiveTurn" class="button secondary" :disabled="busy" @click="controlLiveTurn('steer')"><UiIcon name="send" :size="14"/>追加到当前 Turn</button>
            <button v-if="canInterruptSession" class="button secondary" :disabled="busy" @click="controlLiveTurn('interrupt')"><UiIcon name="x" :size="14"/>中断后追加</button>
            <button class="button secondary" :disabled="busy || !canForkSession" @click="forkSession"><UiIcon name="git-branch" :size="14"/>创建 Session 分支并 Push</button>
            <small>{{ canSteerLiveTurn || canInterruptSession ? '默认 Push 仍会安全排队；中断或 Steer 只在你明确选择时执行。' : canForkSession ? `复制 ${selectedBinding.provider === 'codex' ? 'Codex' : 'OpenCode'} 会话历史并生成新的 Session ID；这不会创建 Git 分支。` : '当前 Session 不可执行这些显式操作。' }}</small>
          </div>
          <div class="session-binding-actions">
            <button class="button secondary manual-bind-toggle" @click="showManual = !showManual; showEdit = false"><UiIcon name="plus" :size="14"/>手动绑定 Session ID</button>
            <button class="button secondary" :disabled="!selectedBinding || selectedSessionIsActive" @click="beginEdit"><UiIcon name="edit" :size="14"/>编辑所选绑定</button>
          </div>
          <p v-if="selectedBinding && selectedSessionIsActive" class="branch-warning"><UiIcon name="alert" :size="14"/>运行中的 Session 不能修改名称或 ID。</p>
          <form v-if="showManual" class="manual-session-form" @submit.prevent="bindManual">
            <label class="field"><span>Profile</span><select v-model="selectedProfile"><option v-for="entry in agents.profiles.filter((item) => item.profile.launch_mode !== 'clipboard_only')" :key="entry.profile.id" :value="entry.profile.id">{{ entry.profile.display_name }}</option></select></label>
            <label class="field"><span>Session ID</span><input v-model="manualSessionId" required placeholder="例如 thr_… 或 UUID"/></label>
            <label class="field"><span>显示名称（可选）</span><input v-model="manualName" placeholder="例如 TASK-001 修复会话"/></label>
            <button class="button secondary" :disabled="busy || !manualSessionId.trim() || !selectedProfile">保存为未验证绑定</button>
          </form>
          <form v-if="showEdit && selectedBinding" class="manual-session-form" @submit.prevent="saveEdit">
            <label class="field"><span>Profile（不可更换）</span><input :value="selectedBinding.profile_id" disabled/></label>
            <label class="field"><span>Session ID</span><input v-model="editSessionId" required autocomplete="off"/></label>
            <label class="field"><span>显示名称（可选）</span><input v-model="editName" placeholder="例如 TASK-001 修复会话"/></label>
            <label v-if="editChangesManagedId" class="confirm-session-replacement"><input v-model="confirmReplacement" type="checkbox"/><span>我确认替换 AuraPilot 自动记录的 Session ID；原 Provider Session 不会被删除。</span></label>
            <div class="session-edit-footer">
              <button type="button" class="button secondary" @click="showEdit = false">取消</button>
              <button class="button secondary" :disabled="busy || !editSessionId.trim() || (editChangesManagedId && !confirmReplacement)">保存并重新验证</button>
            </div>
          </form>
        </template>

        <section v-if="preview" class="prompt-preview"><div><strong>Pointer Prompt</strong><span>{{ preview.text.length }} 字符</span></div><pre>{{ preview.text }}</pre></section>
        <section v-if="outcome" :class="['push-result', outcome.attempt.status === 'failed_to_start' ? 'warning' : 'success']" role="status" aria-live="polite">
          <strong>{{ outcome.message }}</strong>
          <div v-if="outcome.session" class="push-result-target">
            <span>投递目标</span>
            <b>{{ outcome.session.display_name || `${outcome.session.profile_id} Session` }}</b>
            <code :title="outcome.session.external_session_id">{{ shortId(outcome.session.external_session_id) }}</code>
          </div>
          <small v-if="mode === 'existing' && outcome.session?.provider === 'codex'">这是由 AuraPilot 独立连接创建的后台 Turn，不会显示在当前 Codex CLI。请按上方 Thread ID 核对，并在“执行中心”查看实时事件；任务活动日志仍由 Agent 写入任务文件。</small>
        </section>
        <p v-if="branchResult" :class="['push-result', branchResultSuccess ? 'success' : 'warning']" role="status">{{ branchResult }}</p>
        <p v-if="error || agents.error" class="form-error" role="alert" aria-live="assertive">{{ error || agents.error }}</p>
      </div>
      <footer><span class="push-safety">Push 不会领取任务或修改任务状态</span><button class="button secondary" @click="$emit('close')">关闭</button><button class="button primary" :disabled="busy || !canSubmit" @click="push"><UiIcon name="send" :size="15"/>{{ primaryLabel }}</button></footer>
    </section>
  </div>
</template>
