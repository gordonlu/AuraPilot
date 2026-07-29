<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useAgentsStore } from '../stores/agents'
import type { LocatedTask, PointerPrompt, ProjectSnapshot, PushOutcome } from '../types/protocol'
import UiIcon from './UiIcon.vue'

const props = defineProps<{ task: LocatedTask; project: ProjectSnapshot }>()
defineEmits<{ close: [] }>()
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

const selectedEntry = computed(() => agents.profiles.find((entry) => entry.profile.id === selectedProfile.value))
const selectedBinding = computed(() => agents.sessions.find((session) => session.id === selectedSession.value))
const canForkSession = computed(() => selectedBinding.value?.provider === 'codex'
  && ['idle', 'not_loaded'].includes(selectedBinding.value.state))
const canControlLiveTurn = computed(() => selectedBinding.value?.provider === 'codex'
  && selectedBinding.value.state === 'running' && Boolean(selectedBinding.value.active_turn_id))
const copiesOnly = computed(() => selectedEntry.value?.profile.launch_mode === 'clipboard_only')
const primaryLabel = computed(() => {
  if (busy.value) return mode.value === 'existing' ? '正在追加…' : copiesOnly.value ? '正在复制…' : '正在创建并绑定…'
  if (mode.value === 'existing') return 'Push 到所选 Session'
  if (copiesOnly.value) return '复制任务指令'
  return `新建 Session 并 Push 给 ${selectedEntry.value?.profile.display_name ?? 'Agent'}`
})
const canSubmit = computed(() => mode.value === 'existing' ? Boolean(selectedBinding.value) : Boolean(selectedEntry.value))
const shortId = (value: string) => value.length > 22 ? `${value.slice(0, 12)}…${value.slice(-7)}` : value
const formatTime = (value: string) => new Date(value).toLocaleString('zh-CN', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })

onMounted(async () => {
  try {
    await Promise.all([agents.load(), agents.loadSessions(props.project.registration.id)])
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

const applySessionOutcome = (result: PushOutcome) => {
  if (!result.session) return
  const index = agents.sessions.findIndex((item) => item.id === result.session?.id)
  if (index >= 0) agents.sessions[index] = result.session
  else agents.sessions.unshift(result.session)
  selectedSession.value = result.session.id
}

const push = async () => {
  if (!props.task.document.id || !canSubmit.value) return
  busy.value = true; error.value = null; outcome.value = null
  try {
    outcome.value = mode.value === 'existing'
      ? await agents.pushExisting(props.project.registration.id, props.task.document.id, selectedSession.value)
      : await agents.push(props.project.registration.id, props.task.document.id, selectedProfile.value)
    if (mode.value === 'new') props.project.registration.last_profile_id = selectedProfile.value
    applySessionOutcome(outcome.value)
  } catch (caught) { error.value = String(caught) }
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
  if (!props.task.document.id || !selectedBinding.value || !canControlLiveTurn.value) return
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
        <div><span class="modal-mark"><UiIcon name="send"/></span><div><h2>Push {{ task.document.id }}</h2><p>由你选择新 Session 或项目已有 Session</p></div></div>
        <button class="icon-button" aria-label="关闭" @click="$emit('close')"><UiIcon name="x"/></button>
      </header>
      <div class="modal-body">
        <div class="push-mode-switch" role="tablist" aria-label="Session 选择">
          <button :class="{ active: mode === 'new' }" role="tab" :aria-selected="mode === 'new'" @click="mode = 'new'">新 Session</button>
          <button :class="{ active: mode === 'existing' }" role="tab" :aria-selected="mode === 'existing'" @click="mode = 'existing'">项目已有 Session <b>{{ agents.sessions.length }}</b></button>
        </div>

        <template v-if="mode === 'new'">
          <p class="push-notice"><strong>创建新的 Agent Session。</strong>Session 会锁定所选 Profile；“复制任务指令”不会创建 Run 或 Session。</p>
          <div v-if="agents.loading" class="inline-loading">正在检测 Agent…</div>
          <div v-else class="agent-grid">
            <button v-for="entry in agents.profiles" :key="entry.profile.id" :class="['agent-option', { selected: selectedProfile === entry.profile.id }]" @click="selectedProfile = entry.profile.id">
              <span :class="['availability-dot', { available: entry.availability.available }]"/>
              <strong>{{ entry.profile.display_name }}</strong>
              <small>{{ entry.availability.available ? entry.availability.detail : '未检测到，可使用剪贴板兜底' }}</small>
              <b>{{ entry.profile.launch_mode === 'clipboard_only' ? '仅复制' : '新建 Session' }}</b>
            </button>
          </div>
        </template>

        <template v-else>
          <p class="push-notice"><strong>继续项目已有 Session。</strong>AuraPilot 无法保证会话上下文仍适合当前任务，请检查 Profile、项目和 Session ID。默认在安全边界追加。</p>
          <div v-if="agents.sessions.length" class="session-list">
            <button v-for="session in agents.sessions" :key="session.id" :class="['session-option', { selected: selectedSession === session.id }]" @click="selectedSession = session.id">
              <span :class="['session-state', session.state]"/>
              <span><strong>{{ session.display_name || `${session.profile_id} Session` }}</strong><small>{{ session.profile_id }} · {{ session.provider }} · {{ formatTime(session.last_used_at) }}</small></span>
              <code>{{ shortId(session.external_session_id) }}</code>
              <b :class="session.verification">{{ session.verification === 'verified' ? '已验证' : session.verification === 'unavailable' ? '不可用' : '未验证' }}</b>
            </button>
          </div>
          <div v-else class="session-empty"><strong>这个项目还没有已记录的 Session</strong><span>可以先新建 Session，或手动绑定已有 ID。</span></div>
          <div v-if="selectedBinding?.provider === 'codex'" class="session-actions">
            <button v-if="canControlLiveTurn" class="button secondary" :disabled="busy" @click="controlLiveTurn('steer')"><UiIcon name="send" :size="14"/>追加到当前 Turn</button>
            <button v-if="canControlLiveTurn" class="button secondary" :disabled="busy" @click="controlLiveTurn('interrupt')"><UiIcon name="x" :size="14"/>中断后追加</button>
            <button class="button secondary" :disabled="busy || !canForkSession" @click="forkSession"><UiIcon name="git-branch" :size="14"/>创建 Session 分支并 Push</button>
            <small>{{ canControlLiveTurn ? '默认 Push 仍会安全排队；以上操作只在你明确选择时执行。' : canForkSession ? '复制 Codex 会话历史并生成新的 Session ID；这不会创建 Git 分支。' : '当前 Session 不可执行这些显式操作。' }}</small>
          </div>
          <button class="button secondary manual-bind-toggle" @click="showManual = !showManual"><UiIcon name="plus" :size="14"/>手动绑定 Session ID</button>
          <form v-if="showManual" class="manual-session-form" @submit.prevent="bindManual">
            <label class="field"><span>Profile</span><select v-model="selectedProfile"><option v-for="entry in agents.profiles.filter((item) => item.profile.launch_mode !== 'clipboard_only')" :key="entry.profile.id" :value="entry.profile.id">{{ entry.profile.display_name }}</option></select></label>
            <label class="field"><span>Session ID</span><input v-model="manualSessionId" required placeholder="例如 thr_… 或 UUID"/></label>
            <label class="field"><span>显示名称（可选）</span><input v-model="manualName" placeholder="例如 TASK-001 修复会话"/></label>
            <button class="button secondary" :disabled="busy || !manualSessionId.trim() || !selectedProfile">保存为未验证绑定</button>
          </form>
        </template>

        <section v-if="preview" class="prompt-preview"><div><strong>Pointer Prompt</strong><span>{{ preview.text.length }} 字符</span></div><pre>{{ preview.text }}</pre></section>
        <p v-if="outcome" :class="['push-result', outcome.attempt.status === 'failed_to_start' ? 'warning' : 'success']" role="status" aria-live="polite">{{ outcome.message }}</p>
        <p v-if="error || agents.error" class="form-error" role="alert" aria-live="assertive">{{ error || agents.error }}</p>
      </div>
      <footer><span class="push-safety">Push 不会领取任务或修改任务状态</span><button class="button secondary" @click="$emit('close')">关闭</button><button class="button primary" :disabled="busy || !canSubmit" @click="push"><UiIcon name="send" :size="15"/>{{ primaryLabel }}</button></footer>
    </section>
  </div>
</template>
