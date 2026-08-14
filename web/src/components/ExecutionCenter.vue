<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue'
import { useAgentsStore } from '../stores/agents'
import type { ProjectSnapshot, PushAttempt } from '../types/protocol'
import UiIcon from './UiIcon.vue'

const props = defineProps<{ snapshots: ProjectSnapshot[]; activeProject: string; focusApprovalId?: string | null }>()
const emit = defineEmits<{ close: []; openTask: [projectId: string, taskId: string] }>()
const agents = useAgentsStore()
const projectFilter = ref(props.activeProject === 'all' ? 'all' : props.activeProject)
const view = ref<'all' | 'active' | 'issues'>('all')
const query = ref('')

const projectName = (projectId: string) => {
  const snapshot = props.snapshots.find((item) => item.registration.id === projectId)
  return snapshot?.project?.name ?? snapshot?.registration.path.split(/[\\/]/).filter(Boolean).at(-1) ?? '未知项目'
}
const profileLabel = (profileId: string) => agents.profiles.find((item) => item.profile.id === profileId)?.profile.display_name ?? profileId
const shortId = (value: string | null) => !value ? '—' : value.length > 24 ? `${value.slice(0, 13)}…${value.slice(-7)}` : value
const formatTime = (value: string) => new Date(value).toLocaleString('zh-CN', {
  month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit',
})
const attemptLabel = (attempt: PushAttempt) => ({
  requested: '等待投递', started: 'Agent 已接收', failed_to_start: '启动失败', exited: attempt.error ? '异常结束' : '进程已结束', status_unknown: '重启后状态未知',
}[attempt.status])

const visibleEvents = computed(() => {
  const search = query.value.trim().toLowerCase()
  return agents.executionEvents.filter((event) => {
    if (projectFilter.value !== 'all' && event.project_id !== projectFilter.value) return false
    if (view.value === 'issues' && !['warning', 'error'].includes(event.level)) return false
    if (view.value === 'active') {
      const attempt = agents.pushAttempts.find((item) => item.id === event.attempt_id)
      if (!attempt || !['requested', 'started'].includes(attempt.status)) return false
    }
    return !search || [event.task_id, event.profile_id, event.message, event.phase, event.detail, projectName(event.project_id)]
      .some((value) => value?.toLowerCase().includes(search))
  })
})
const visibleAttempts = computed(() => agents.pushAttempts.filter((attempt) => {
  if (projectFilter.value !== 'all' && attempt.project_id !== projectFilter.value) return false
  if (view.value === 'active') return ['requested', 'started'].includes(attempt.status)
  if (view.value === 'issues') return ['failed_to_start', 'status_unknown'].includes(attempt.status) || Boolean(attempt.error)
  const search = query.value.trim().toLowerCase()
  return !search || [attempt.task_id, attempt.agent_profile_id, attempt.error, projectName(attempt.project_id)]
    .some((value) => value?.toLowerCase().includes(search))
}))
const visibleApprovals = computed(() => agents.approvals.filter((approval) => {
  if (projectFilter.value !== 'all' && approval.project_id !== projectFilter.value) return false
  if (view.value === 'active') return ['pending', 'submitting'].includes(approval.status)
  if (view.value === 'issues') return ['pending', 'failed'].includes(approval.status)
  const search = query.value.trim().toLowerCase()
  return !search || [approval.task_id, approval.command, approval.cwd, approval.reason, projectName(approval.project_id)]
    .some((value) => value?.toLowerCase().includes(search))
}))
const activeCount = computed(() => agents.pushAttempts.filter((attempt) => ['requested', 'started'].includes(attempt.status)).length)
const issueCount = computed(() => agents.pushAttempts.filter((attempt) => ['failed_to_start', 'status_unknown'].includes(attempt.status) || attempt.error).length
  + agents.executionEvents.filter((event) => event.kind !== 'approval' && ['warning', 'error'].includes(event.level)).length
  + agents.approvals.filter((approval) => ['pending', 'failed'].includes(approval.status)).length)

const refresh = () => Promise.all([
  agents.loadExecutionEvents(projectFilter.value === 'all' ? undefined : projectFilter.value),
  agents.loadApprovals(projectFilter.value === 'all' ? undefined : projectFilter.value),
])
const canOpenTask = (projectId: string, taskId: string) => props.snapshots
  .find((snapshot) => snapshot.registration.id === projectId)?.tasks
  .some((task) => task.document.id === taskId)

onMounted(async () => {
  await refresh().catch(() => undefined)
  await nextTick()
  if (props.focusApprovalId) document.querySelector(`[data-approval-id="${CSS.escape(props.focusApprovalId)}"]`)?.scrollIntoView({ block: 'center' })
})
</script>

<template>
  <div class="modal-backdrop execution-backdrop" @mousedown.self="$emit('close')">
    <section class="execution-center" role="dialog" aria-modal="true" aria-label="Agent 执行中心">
      <header class="execution-header">
        <div><span class="modal-mark"><UiIcon name="terminal"/></span><div><h2>Agent 执行中心</h2><p>Provider 运行事件与 Push 生命周期，不能替代任务文件状态</p></div></div>
        <button class="icon-button" aria-label="关闭" @click="$emit('close')"><UiIcon name="x"/></button>
      </header>

      <div class="execution-toolbar">
        <label class="execution-project"><span>项目</span><select v-model="projectFilter" @change="refresh">
          <option value="all">所有项目</option>
          <option v-for="snapshot in snapshots" :key="snapshot.registration.id" :value="snapshot.registration.id">{{ projectName(snapshot.registration.id) }}</option>
        </select></label>
        <label class="execution-search"><UiIcon name="search" :size="14"/><input v-model="query" placeholder="筛选任务、事件、错误…"/></label>
        <button class="button secondary" :disabled="agents.executionLoading" @click="refresh"><UiIcon name="diagnostic" :size="14"/>{{ agents.executionLoading ? '刷新中…' : '刷新' }}</button>
      </div>

      <nav class="execution-tabs" aria-label="执行记录筛选">
        <button :class="{ active: view === 'all' }" @click="view = 'all'">全部 <b>{{ visibleEvents.length + visibleAttempts.length + visibleApprovals.length }}</b></button>
        <button :class="{ active: view === 'active' }" @click="view = 'active'">进行中 <b>{{ activeCount }}</b></button>
        <button :class="{ active: view === 'issues' }" @click="view = 'issues'">需要处理 <b :class="{ danger: issueCount > 0 }">{{ issueCount }}</b></button>
      </nav>
      <p class="execution-coverage"><strong>当前可见范围：</strong>Codex 提供实时 Provider 事件；OpenCode 提供运行生命周期与最终回复；外部交互式 CLI 只能确认进程启动和退出。没有回传的数据不会被模拟。</p>

      <div class="execution-content">
        <div v-if="agents.executionError" class="execution-error" role="alert"><UiIcon name="alert" :size="15"/><span>{{ agents.executionError }}</span><button @click="refresh">重试</button></div>
        <div v-if="agents.approvalError" class="execution-error" role="alert"><UiIcon name="alert" :size="15"/><span>{{ agents.approvalError }}</span><button @click="refresh">刷新状态</button></div>
        <section v-if="visibleApprovals.length" class="approval-panel" aria-label="Codex 审批请求">
          <div class="execution-section-title"><strong>Codex 审批请求</strong><span>决定会发回原 Session 连接</span></div>
          <article v-for="approval in visibleApprovals" :key="approval.id" :data-approval-id="approval.id" :class="['approval-card', approval.status, { focused: approval.id === focusApprovalId }]">
            <header><strong>{{ approval.kind === 'command_execution' ? '命令执行' : '文件变更' }}</strong><b>{{ approval.status === 'pending' ? '等待处理' : approval.status === 'submitting' ? '正在提交' : approval.status === 'approved' ? '已批准' : approval.status === 'declined' ? '已拒绝' : approval.status === 'expired' ? '已失效' : '处理失败' }}</b></header>
            <p>{{ approval.command || 'Codex 请求应用文件变更' }}</p>
            <small>{{ projectName(approval.project_id) }} · {{ approval.task_id || '未关联任务' }} · {{ profileLabel(approval.profile_id) }}</small>
            <small v-if="approval.cwd">目录：{{ approval.cwd }}</small>
            <small v-if="approval.reason">原因：{{ approval.reason }}</small>
            <p v-if="approval.error" class="approval-error">{{ approval.error }}</p>
            <div v-if="approval.status === 'pending'" class="approval-actions">
              <button class="button secondary" @click="agents.respondApproval(approval.id, 'decline').catch(() => undefined)">拒绝</button>
              <button class="button primary" @click="agents.respondApproval(approval.id, 'accept').catch(() => undefined)">批准并继续</button>
            </div>
          </article>
        </section>
        <div class="execution-body">
        <aside class="execution-attempts">
          <div class="execution-section-title"><strong>Push / Run</strong><span>{{ visibleAttempts.length }}</span></div>
          <button
            v-for="attempt in visibleAttempts" :key="attempt.id"
            :class="['execution-attempt', attempt.status, { error: attempt.error }]"
            :disabled="!canOpenTask(attempt.project_id, attempt.task_id)"
            @click="$emit('openTask', attempt.project_id, attempt.task_id)"
          >
            <span class="execution-state-dot"/>
            <span><strong>{{ attempt.task_id }}</strong><small>{{ projectName(attempt.project_id) }} · {{ profileLabel(attempt.agent_profile_id) }}</small></span>
            <b>{{ attemptLabel(attempt) }}</b>
            <time>{{ formatTime(attempt.created_at) }}</time>
            <p v-if="attempt.error">{{ attempt.error }}</p>
          </button>
          <p v-if="!visibleAttempts.length" class="execution-empty-small">没有符合条件的 Push。</p>
        </aside>

        <section class="execution-timeline">
          <div class="execution-section-title"><strong>Provider 时间线</strong><span>本地持久化 · 最近 {{ visibleEvents.length }} 条</span></div>
          <article v-for="event in visibleEvents" :key="event.id" :class="['execution-event', event.level, event.kind]">
            <span class="execution-event-icon"><UiIcon :name="event.kind === 'command' ? 'terminal' : event.kind === 'file_change' ? 'edit' : ['warning','error'].includes(event.level) ? 'alert' : 'diagnostic'" :size="14"/></span>
            <div class="execution-event-content">
              <header><button :disabled="!canOpenTask(event.project_id, event.task_id)" @click="$emit('openTask', event.project_id, event.task_id)">{{ event.task_id }}</button><span>{{ projectName(event.project_id) }}</span><b>{{ profileLabel(event.profile_id) }}</b><time>{{ formatTime(event.created_at) }}</time></header>
              <p>{{ event.message }}</p>
              <small>{{ event.phase }}<template v-if="event.session_binding_id"> · Session {{ shortId(event.session_binding_id) }}</template></small>
              <details v-if="event.detail"><summary>查看 Provider 原始详情</summary><pre>{{ event.detail }}</pre></details>
            </div>
          </article>
          <div v-if="!visibleEvents.length && !agents.executionLoading" class="execution-empty"><UiIcon name="terminal" :size="28"/><strong>还没有 Provider 运行事件</strong><p>后台 Push 后，这里会显示启动、命令、文件变更、审批、完成与失败事件。普通外部 CLI 无法回传的内容会明确保持为空。</p></div>
        </section>
        </div>
      </div>

      <footer class="execution-footer"><span><i/>事件保存在 AuraPilot 本地数据库；任务 YAML 仍是任务状态唯一事实来源。</span><button class="button secondary" @click="$emit('close')">关闭</button></footer>
    </section>
  </div>
</template>
