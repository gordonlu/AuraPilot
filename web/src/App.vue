<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import AddProjectModal from './components/AddProjectModal.vue'
import AgentProfilesModal from './components/AgentProfilesModal.vue'
import AuraTransferModal from './components/AuraTransferModal.vue'
import AppSidebar from './components/AppSidebar.vue'
import BlockedView from './components/BlockedView.vue'
import BoardView from './components/BoardView.vue'
import ConfirmDialog from './components/ConfirmDialog.vue'
import DiagnosticsPanel from './components/DiagnosticsPanel.vue'
import EmptyState from './components/EmptyState.vue'
import TaskDrawer from './components/TaskDrawer.vue'
import TaskFormModal from './components/TaskFormModal.vue'
import PushTaskModal from './components/PushTaskModal.vue'
import ProjectsView from './components/ProjectsView.vue'
import WorldSkinPickerModal from './components/WorldSkinPickerModal.vue'
import WorldSkinHost from './skins/WorldSkinHost.vue'
import UiIcon from './components/UiIcon.vue'
import { useProjectsStore } from './stores/projects'
import { useAgentsStore } from './stores/agents'
import {
  resolveWorldSkin,
  WORLD_SKIN_STORAGE_KEY,
  type WorldSkin,
} from './skins/worldSkin'
import type { WorldSkinRuntimeState } from './skins/runtime'
import type { PetEventSignal } from './skins/runtime'
import type { PetDialogueContext } from './skins/pets/dialogue'
import type { PetEventId } from './skins/pets/manifest'
import {
  hasNewlyBlockedTask,
  snapshotBlockedTasks,
  transitionPetEvent,
  type BlockedTaskState,
} from './skins/pets/signals'
import { nextTheme, resolveTheme, type Theme } from './theme'
import type { LocatedTask, TaskDraft, TaskTransition } from './types/protocol'

const projectsStore = useProjectsStore()
const agentsStore = useAgentsStore()
const activeProject = ref('all')
const activeView = ref<'projects' | 'board' | 'blocked'>('projects')
const search = ref('')
const theme = ref<Theme>(resolveTheme(localStorage.getItem('aurapilot-theme')))
const worldSkin = ref<WorldSkin>(resolveWorldSkin(localStorage.getItem(WORLD_SKIN_STORAGE_KEY)))
const worldSkinState = ref<WorldSkinRuntimeState>({ skin: worldSkin.value, status: 'idle', error: null })
const worldSkinError = ref<string | null>(null)
const petEvent = ref<PetEventSignal | null>(null)
let petEventSequence = 0
let blockedTaskState: BlockedTaskState = new Map()
let blockedTaskBaselineReady = false
const selected = ref<{ projectId: string; taskId: string } | null>(null)
const modal = ref<'create' | 'edit' | 'add-project' | 'delete' | 'push' | 'profiles' | 'transfer' | 'world-skins' | null>(null)
const showDiagnostics = ref(false)
const busy = ref(false)
const actionError = ref<string | null>(null)
const lastRefresh = ref(new Date())
const projectPath = ref('')
const projectSelecting = ref(false)
const projectCanInitialize = ref(false)

const allSnapshots = computed(() => Object.values(projectsStore.snapshots))
const visibleSnapshots = computed(() => activeProject.value === 'all'
  ? allSnapshots.value
  : allSnapshots.value.filter((item) => item.registration.id === activeProject.value))
const contextSnapshots = computed(() => activeView.value === 'projects' ? allSnapshots.value : visibleSnapshots.value)
const diagnosticCount = computed(() => contextSnapshots.value.reduce((sum, item) => sum + item.diagnostics.length, 0))
const taskCount = computed(() => contextSnapshots.value.reduce((sum, item) => sum + item.tasks.length, 0))
const petContext = computed<PetDialogueContext>(() => {
  const tasks = allSnapshots.value.flatMap((snapshot) => snapshot.tasks)
  return {
    projects: allSnapshots.value.length,
    backlog: tasks.filter((task) => task.state === 'backlog').length,
    inProgress: tasks.filter((task) => task.state === 'in-progress').length,
    inReview: tasks.filter((task) => task.state === 'in-review').length,
    done: tasks.filter((task) => task.state === 'done').length,
    blocked: tasks.filter((task) => task.document.blockers.length > 0).length,
  }
})
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
const openProjectBoard = (projectId: string) => {
  activeProject.value = projectId
  activeView.value = 'board'
  search.value = ''
}
const closeOverlays = () => { modal.value = null; selected.value = null; showDiagnostics.value = false; actionError.value = null }
const toggleTheme = () => {
  theme.value = nextTheme(theme.value)
  localStorage.setItem('aurapilot-theme', theme.value)
  document.documentElement.dataset.theme = theme.value
}
const setWorldSkin = (skin: WorldSkin) => {
  worldSkin.value = skin
  localStorage.setItem(WORLD_SKIN_STORAGE_KEY, skin)
  document.documentElement.dataset.worldSkin = skin
}
const selectWorldSkin = (skin: WorldSkin) => {
  worldSkinError.value = null
  setWorldSkin(skin)
}
const onWorldSkinState = (state: WorldSkinRuntimeState) => {
  worldSkinState.value = state
  if (state.status === 'ready') worldSkinError.value = null
  if (state.status !== 'error') return
  worldSkinError.value = state.error ?? '未知错误'
  setWorldSkin('classic')
}
const retryWorldSkin = () => {
  worldSkinError.value = null
  setWorldSkin('seascape')
}
const signalPet = (event: PetEventId) => {
  petEvent.value = { sequence: ++petEventSequence, event }
}
watch(allSnapshots, (snapshots) => {
  const next = snapshotBlockedTasks(snapshots)
  if (blockedTaskBaselineReady && hasNewlyBlockedTask(blockedTaskState, next)) {
    signalPet('task-blocked')
  }
  blockedTaskState = next
  blockedTaskBaselineReady = true
}, { deep: true })
watch(() => projectsStore.error, (error, previous) => {
  if (error && !previous) signalPet('sync-failed')
})
const openAddProject = () => {
  projectPath.value = ''
  projectCanInitialize.value = false
  actionError.value = null
  modal.value = 'add-project'
}
const updateProjectPath = (path: string) => {
  projectPath.value = path
  projectCanInitialize.value = false
  actionError.value = null
}
const projectErrorMessage = (error: unknown, action = '添加') => {
  const message = String(error)
  if (message.includes('already registered')) return '这个项目已经在 AuraPilot 中。'
  if (message.includes('No such file') || message.includes('not a directory')) return '所选目录不存在或无法访问，请重新选择。'
  return `无法${action}项目：${message}`
}
const chooseProjectDirectory = async () => {
  projectSelecting.value = true
  actionError.value = null
  try {
    const selectedPath = await projectsStore.chooseDirectory()
    if (selectedPath) updateProjectPath(selectedPath)
  } catch (error) { actionError.value = projectErrorMessage(error) }
  finally { projectSelecting.value = false }
}
const saveTask = async (projectId: string, draft: TaskDraft) => {
  busy.value = true; actionError.value = null
  try {
    const creating = modal.value !== 'edit'
    if (modal.value === 'edit' && selected.value) await projectsStore.update(projectId, selected.value.taskId, draft)
    else await projectsStore.create(projectId, draft)
    if (creating) signalPet('task-created')
    modal.value = null; lastRefresh.value = new Date()
  } catch (error) { actionError.value = String(error) } finally { busy.value = false }
}
const addProject = async (path: string) => {
  busy.value = true; actionError.value = null
  try { const project = await projectsStore.add(path); activeProject.value = project.id; modal.value = null }
  catch (error) {
    if (String(error).includes('project does not contain a .aurapilot directory')) {
      projectCanInitialize.value = true
    } else actionError.value = projectErrorMessage(error)
  } finally { busy.value = false }
}
const initializeProject = async (path: string) => {
  busy.value = true; actionError.value = null
  try {
    const project = await projectsStore.initialize(path)
    activeProject.value = project.id
    modal.value = null
    lastRefresh.value = new Date()
  } catch (error) { actionError.value = projectErrorMessage(error, '初始化') }
  finally { busy.value = false }
}
const transitionTask = async (input: TaskTransition) => {
  if (!selected.value) return
  busy.value = true; actionError.value = null
  try {
    await projectsStore.transition(selected.value.projectId, selected.value.taskId, input)
    const event = transitionPetEvent(input.target)
    if (event) signalPet(event)
    lastRefresh.value = new Date()
  }
  catch (error) { actionError.value = String(error) } finally { busy.value = false }
}
const deleteTask = async () => {
  if (!selected.value) return
  busy.value = true; actionError.value = null
  try { await projectsStore.deleteTask(selected.value.projectId, selected.value.taskId); closeOverlays(); lastRefresh.value = new Date() }
  catch (error) { actionError.value = String(error) } finally { busy.value = false }
}
const retryProjectSync = async () => {
  await projectsStore.load()
}
const onKeydown = (event: KeyboardEvent) => {
  if (event.key === 'Escape') { closeOverlays(); return }
  const target = event.target as HTMLElement
  if (['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)) return
  if (event.key.toLowerCase() === 'n' && allSnapshots.value.length) modal.value = 'create'
  if (event.key.toLowerCase() === 'b') activeView.value = activeView.value === 'board' ? 'blocked' : 'board'
  if (event.key === '/') { event.preventDefault(); document.querySelector<HTMLInputElement>('#task-search')?.focus() }
}

onMounted(async () => {
  document.documentElement.dataset.theme = theme.value
  await agentsStore.startWatchingAttempts()
  await projectsStore.load()
  await projectsStore.startWatching()
  if (import.meta.env.DEV) {
    const preview = new URLSearchParams(window.location.search)
    if (preview.get('view') === 'blocked') activeView.value = 'blocked'
    if (preview.get('view') === 'projects') activeView.value = 'projects'
    if (preview.get('modal') === 'add-project') {
      openAddProject()
      projectPath.value = preview.get('path') ?? ''
      projectCanInitialize.value = preview.get('needsInit') === '1'
    }
    const taskId = preview.get('task')
    if (taskId) {
      const project = allSnapshots.value.find((item) => item.tasks.some((task) => task.document.id === taskId))
      if (project) selected.value = { projectId: project.registration.id, taskId }
    }
    if (preview.get('modal') === 'create' && allSnapshots.value.length) modal.value = 'create'
    if (preview.get('modal') === 'transfer' && allSnapshots.value.length) modal.value = 'transfer'
    if (preview.get('modal') === 'push' && selected.value) modal.value = 'push'
    const agentError = preview.get('agentError')
    if (agentError) agentsStore.runtimeError = agentError
  }
  window.addEventListener('keydown', onKeydown)
})
onBeforeUnmount(() => {
  projectsStore.stopWatching()
  agentsStore.stopWatchingAttempts()
  window.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <div
    :class="['app-shell', { 'blocked-atmosphere': activeView === 'blocked' }]"
    :data-theme="theme"
    :data-world-skin="worldSkin"
  >
    <AppSidebar
      :snapshots="allSnapshots" :active-project="activeProject" :active-view="activeView"
      :theme="theme" :world-skin="worldSkin" :diagnostic-count="diagnosticCount"
      @project="activeProject = $event" @view="activeView = $event" @add="openAddProject"
      @theme="toggleTheme" @diagnostics="showDiagnostics = !showDiagnostics" @profiles="modal = 'profiles'"
      @world-skin="modal = 'world-skins'" @transfer="modal = 'transfer'"
    />
    <main class="workspace">
      <header class="app-toolbar">
        <div class="current-project"><UiIcon name="folder"/><span>{{ activeView === 'projects' ? '项目一览' : activeProject === 'all' ? '所有项目' : visibleSnapshots[0]?.project?.name }}</span><b>{{ activeView === 'projects' ? allSnapshots.length : taskCount }}</b></div>
        <label class="search-box"><UiIcon name="search"/><input id="task-search" v-model="search" :placeholder="activeView === 'projects' ? '搜索项目名称或路径…' : '搜索任务或 ID…'"/><kbd>/</kbd></label>
        <div class="view-switch"><button :class="{ active: activeView === 'projects' }" @click="activeView = 'projects'"><UiIcon name="folder"/>项目</button><button :class="{ active: activeView === 'board' }" @click="activeView = 'board'"><UiIcon name="board"/>看板</button><button :class="{ active: activeView === 'blocked' }" @click="activeView = 'blocked'"><UiIcon name="alert"/>阻塞</button></div>
        <button class="button primary" :disabled="!allSnapshots.length" @click="modal = 'create'"><UiIcon name="plus"/>新建任务</button>
      </header>

      <div v-if="projectsStore.error" class="runtime-error" role="alert">
        <UiIcon name="alert" :size="15"/>
        <span>{{ projectsStore.error }}</span>
        <button @click="retryProjectSync">重新扫描</button>
      </div>
      <div v-if="agentsStore.runtimeError" class="runtime-error" role="alert">
        <UiIcon name="alert" :size="15"/>
        <span>Agent 操作失败：{{ agentsStore.runtimeError }}</span>
        <button @click="agentsStore.clearRuntimeError">知道了</button>
      </div>
      <div v-if="worldSkinError" class="runtime-error" role="alert">
        <UiIcon name="alert" :size="15"/>
        <span>世界皮肤启动失败：{{ worldSkinError }}。已安全回到经典界面。</span>
        <button @click="retryWorldSkin">重试海岸世界</button>
      </div>

      <div class="main-surface">
        <WorldSkinHost
          :skin="worldSkin"
          :event="petEvent"
          :context="petContext"
          @state="onWorldSkinState"
        />
        <div v-if="projectsStore.loading" class="loading-state"><span/><p>正在扫描项目协议…</p></div>
        <EmptyState v-else-if="!allSnapshots.length" @add="openAddProject" />
        <ProjectsView v-else-if="activeView === 'projects'" :snapshots="allSnapshots" :search="search" @open="openProjectBoard" />
        <BoardView v-else-if="activeView === 'board'" :snapshots="visibleSnapshots" :search="search" :selected-key="selectedKey" @open="selectTask" />
        <BlockedView v-else :snapshots="visibleSnapshots" :search="search" @open="selectTask" @back="activeView = 'board'" />
      </div>

      <footer class="status-bar"><span class="live"><i :class="{ warning: projectsStore.error }"/>{{ projectsStore.error ? '同步需要处理' : 'Watcher 实时监控中' }}</span><span>项目 {{ contextSnapshots.length }}</span><span>任务 {{ taskCount }}</span><span>最后刷新 {{ lastRefresh.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' }) }}</span><button v-if="diagnosticCount" @click="showDiagnostics = true"><UiIcon name="alert" :size="14"/>{{ diagnosticCount }} 条诊断</button></footer>
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
    <AddProjectModal
      v-if="modal === 'add-project'" :path="projectPath" :busy="busy" :selecting="projectSelecting"
      :error="actionError" :can-initialize="projectCanInitialize"
      @update:path="updateProjectPath" @browse="chooseProjectDirectory" @add="addProject"
      @initialize="initializeProject" @close="modal = null; actionError = null"
    />
    <PushTaskModal
      v-if="modal === 'push' && selectedTask && selectedProject"
      :task="selectedTask" :project="selectedProject"
      @close="modal = null" @pet-event="signalPet"
    />
    <AgentProfilesModal v-if="modal === 'profiles'" :projects="allSnapshots" @close="modal = null" />
    <AuraTransferModal
      v-if="modal === 'transfer'" :projects="allSnapshots"
      :initial-project-id="activeProject" @close="modal = null"
    />
    <WorldSkinPickerModal
      v-if="modal === 'world-skins'"
      :current="worldSkin"
      :runtime-state="worldSkinState"
      :error="worldSkinError"
      @select="selectWorldSkin"
      @close="modal = null"
    />
    <ConfirmDialog
      v-if="modal === 'delete' && selectedTask" title="删除任务？"
      :message="`${selectedTask.document.id} 将从 .aurapilot 中永久删除。项目内其他文件不会受影响。`"
      :busy="busy" :error="actionError" @close="modal = null; actionError = null" @confirm="deleteTask"
    />
  </div>
</template>
