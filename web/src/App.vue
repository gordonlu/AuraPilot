<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import AddProjectModal from './components/AddProjectModal.vue'
import AgentProfilesModal from './components/AgentProfilesModal.vue'
import AppSidebar from './components/AppSidebar.vue'
import BlockedView from './components/BlockedView.vue'
import BoardView from './components/BoardView.vue'
import ConfirmDialog from './components/ConfirmDialog.vue'
import DiagnosticsPanel from './components/DiagnosticsPanel.vue'
import EmptyState from './components/EmptyState.vue'
import TaskDrawer from './components/TaskDrawer.vue'
import TaskFormModal from './components/TaskFormModal.vue'
import PushTaskModal from './components/PushTaskModal.vue'
import UiIcon from './components/UiIcon.vue'
import { useProjectsStore } from './stores/projects'
import type { LocatedTask, TaskDraft, TaskTransition } from './types/protocol'

const projectsStore = useProjectsStore()
const activeProject = ref('all')
const activeView = ref<'board' | 'blocked'>('board')
const search = ref('')
const theme = ref<'dark' | 'light'>((localStorage.getItem('aurapilot-theme') as 'dark' | 'light') || 'dark')
const selected = ref<{ projectId: string; taskId: string } | null>(null)
const modal = ref<'create' | 'edit' | 'add-project' | 'delete' | 'push' | 'profiles' | null>(null)
const showDiagnostics = ref(false)
const busy = ref(false)
const actionError = ref<string | null>(null)
const lastRefresh = ref(new Date())

const allSnapshots = computed(() => Object.values(projectsStore.snapshots))
const visibleSnapshots = computed(() => activeProject.value === 'all'
  ? allSnapshots.value
  : allSnapshots.value.filter((item) => item.registration.id === activeProject.value))
const diagnosticCount = computed(() => visibleSnapshots.value.reduce((sum, item) => sum + item.diagnostics.length, 0))
const taskCount = computed(() => visibleSnapshots.value.reduce((sum, item) => sum + item.tasks.length, 0))
const selectedProject = computed(() => selected.value ? projectsStore.snapshots[selected.value.projectId] : null)
const selectedTask = computed(() => selectedProject.value?.tasks.find((task) => task.document.id === selected.value?.taskId) ?? null)
const selectedDiagnostics = computed(() => selectedTask.value && selectedProject.value
  ? selectedProject.value.diagnostics.filter((item) => item.path === selectedTask.value?.path)
  : [])
const selectedKey = computed(() => selected.value ? `${selected.value.projectId}:${selected.value.taskId}` : null)

const selectTask = (projectId: string, task: LocatedTask) => {
  if (!task.document.id) return
  selected.value = { projectId, taskId: task.document.id }
  actionError.value = null
}
const closeOverlays = () => { modal.value = null; selected.value = null; showDiagnostics.value = false; actionError.value = null }
const toggleTheme = () => {
  theme.value = theme.value === 'dark' ? 'light' : 'dark'
  localStorage.setItem('aurapilot-theme', theme.value)
}
const saveTask = async (projectId: string, draft: TaskDraft) => {
  busy.value = true; actionError.value = null
  try {
    if (modal.value === 'edit' && selected.value) await projectsStore.update(projectId, selected.value.taskId, draft)
    else await projectsStore.create(projectId, draft)
    modal.value = null; lastRefresh.value = new Date()
  } catch (error) { actionError.value = String(error) } finally { busy.value = false }
}
const addProject = async (path: string) => {
  busy.value = true; actionError.value = null
  try { const project = await projectsStore.add(path); activeProject.value = project.id; modal.value = null }
  catch (error) { actionError.value = String(error) } finally { busy.value = false }
}
const transitionTask = async (input: TaskTransition) => {
  if (!selected.value) return
  busy.value = true; actionError.value = null
  try { await projectsStore.transition(selected.value.projectId, selected.value.taskId, input); lastRefresh.value = new Date() }
  catch (error) { actionError.value = String(error) } finally { busy.value = false }
}
const deleteTask = async () => {
  if (!selected.value) return
  busy.value = true; actionError.value = null
  try { await projectsStore.deleteTask(selected.value.projectId, selected.value.taskId); closeOverlays(); lastRefresh.value = new Date() }
  catch (error) { actionError.value = String(error) } finally { busy.value = false }
}
const onKeydown = (event: KeyboardEvent) => {
  const target = event.target as HTMLElement
  if (['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)) return
  if (event.key === 'Escape') closeOverlays()
  if (event.key.toLowerCase() === 'n' && allSnapshots.value.length) modal.value = 'create'
  if (event.key.toLowerCase() === 'b') activeView.value = activeView.value === 'board' ? 'blocked' : 'board'
  if (event.key === '/') { event.preventDefault(); document.querySelector<HTMLInputElement>('#task-search')?.focus() }
}

onMounted(async () => {
  document.documentElement.dataset.theme = theme.value
  await projectsStore.load()
  await projectsStore.startWatching()
  if (import.meta.env.DEV) {
    const preview = new URLSearchParams(window.location.search)
    if (preview.get('view') === 'blocked') activeView.value = 'blocked'
    const taskId = preview.get('task')
    if (taskId) {
      const project = allSnapshots.value.find((item) => item.tasks.some((task) => task.document.id === taskId))
      if (project) selected.value = { projectId: project.registration.id, taskId }
    }
    if (preview.get('modal') === 'create' && allSnapshots.value.length) modal.value = 'create'
  }
  window.addEventListener('keydown', onKeydown)
})
onBeforeUnmount(() => { projectsStore.stopWatching(); window.removeEventListener('keydown', onKeydown) })
</script>

<template>
  <div :class="['app-shell', { 'blocked-atmosphere': activeView === 'blocked' }]" :data-theme="theme">
    <AppSidebar
      :snapshots="allSnapshots" :active-project="activeProject" :active-view="activeView"
      :theme="theme" :diagnostic-count="diagnosticCount"
      @project="activeProject = $event" @view="activeView = $event" @add="modal = 'add-project'"
      @theme="toggleTheme" @diagnostics="showDiagnostics = !showDiagnostics" @profiles="modal = 'profiles'"
    />
    <main class="workspace">
      <header class="app-toolbar">
        <div class="current-project"><UiIcon name="folder"/><span>{{ activeProject === 'all' ? '所有项目' : visibleSnapshots[0]?.project?.name }}</span><b>{{ taskCount }}</b></div>
        <label class="search-box"><UiIcon name="search"/><input id="task-search" v-model="search" placeholder="搜索任务或 ID…"/><kbd>/</kbd></label>
        <div class="view-switch"><button :class="{ active: activeView === 'board' }" @click="activeView = 'board'"><UiIcon name="board"/>看板</button><button :class="{ active: activeView === 'blocked' }" @click="activeView = 'blocked'"><UiIcon name="alert"/>阻塞</button></div>
        <button class="button primary" :disabled="!allSnapshots.length" @click="modal = 'create'"><UiIcon name="plus"/>新建任务</button>
      </header>

      <div class="main-surface">
        <div v-if="projectsStore.loading" class="loading-state"><span/><p>正在扫描项目协议…</p></div>
        <EmptyState v-else-if="!allSnapshots.length" @add="modal = 'add-project'" />
        <BoardView v-else-if="activeView === 'board'" :snapshots="visibleSnapshots" :search="search" :selected-key="selectedKey" @open="selectTask" />
        <BlockedView v-else :snapshots="visibleSnapshots" :search="search" @open="selectTask" @back="activeView = 'board'" />
      </div>

      <footer class="status-bar"><span class="live"><i/>Watcher 实时监控中</span><span>项目 {{ visibleSnapshots.length }}</span><span>任务 {{ taskCount }}</span><span>最后刷新 {{ lastRefresh.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' }) }}</span><button v-if="diagnosticCount" @click="showDiagnostics = true"><UiIcon name="alert" :size="14"/>{{ diagnosticCount }} 条诊断</button></footer>
    </main>

    <TaskDrawer
      v-if="selectedTask && selectedProject" :task="selectedTask" :project="selectedProject"
      :diagnostics="selectedDiagnostics" :busy="busy" :error="actionError"
      @close="selected = null" @edit="modal = 'edit'" @delete="modal = 'delete'" @push="modal = 'push'" @transition="transitionTask"
    />
    <DiagnosticsPanel v-if="showDiagnostics" :snapshots="visibleSnapshots" @close="showDiagnostics = false" />
    <TaskFormModal
      v-if="modal === 'create' || (modal === 'edit' && selectedTask)" :mode="modal === 'edit' ? 'edit' : 'create'"
      :projects="allSnapshots" :project-id="selected?.projectId ?? (activeProject === 'all' ? '' : activeProject)"
      :task="modal === 'edit' ? selectedTask : null" :busy="busy" :error="actionError"
      @close="modal = null; actionError = null" @save="saveTask"
    />
    <AddProjectModal v-if="modal === 'add-project'" :busy="busy" :error="actionError" @close="modal = null; actionError = null" @add="addProject" />
    <PushTaskModal v-if="modal === 'push' && selectedTask && selectedProject" :task="selectedTask" :project="selectedProject" @close="modal = null" />
    <AgentProfilesModal v-if="modal === 'profiles'" :projects="allSnapshots" @close="modal = null" />
    <ConfirmDialog
      v-if="modal === 'delete' && selectedTask" title="删除任务？"
      :message="`${selectedTask.document.id} 将从 .aurapilot 中永久删除。项目内其他文件不会受影响。`"
      :busy="busy" :error="actionError" @close="modal = null; actionError = null" @confirm="deleteTask"
    />
  </div>
</template>
