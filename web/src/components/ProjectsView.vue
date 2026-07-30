<script setup lang="ts">
import { computed, ref } from 'vue'
import { useProjectsStore } from '../stores/projects'
import type { ProjectSnapshot, TaskState } from '../types/protocol'
import UiIcon from './UiIcon.vue'

const props = defineProps<{ snapshots: ProjectSnapshot[]; search: string }>()
const projects = useProjectsStore()
const opening = ref<string | null>(null)
const feedback = ref<{ kind: 'success' | 'error'; text: string } | null>(null)

defineEmits<{ open: [projectId: string] }>()

const states: Array<{ key: TaskState; label: string; short: string }> = [
  { key: 'backlog', label: 'Backlog', short: '待办' },
  { key: 'in-progress', label: 'In Progress', short: '进行中' },
  { key: 'in-review', label: 'In Review', short: '评审中' },
  { key: 'done', label: 'Done', short: '完成' },
]

const projectName = (snapshot: ProjectSnapshot) => snapshot.project?.name
  ?? snapshot.registration.path.split(/[\\/]/).filter(Boolean).at(-1)
  ?? '未命名项目'
const taskCount = (snapshot: ProjectSnapshot, state: TaskState) => snapshot.tasks
  .filter((task) => task.state === state).length
const blockerCount = (snapshot: ProjectSnapshot) => snapshot.tasks
  .filter((task) => task.document.blockers?.length).length
const healthLabel = (health: string | null | undefined) => ({
  green: '健康',
  yellow: '需关注',
  red: '异常',
}[health ?? ''] ?? '未知')

const rows = computed(() => {
  const query = props.search.trim().toLocaleLowerCase()
  return props.snapshots.filter((snapshot) => !query || [
    projectName(snapshot),
    snapshot.registration.path,
  ].some((value) => value?.toLocaleLowerCase().includes(query)))
})

const summary = computed(() => ({
  projects: props.snapshots.length,
  healthy: props.snapshots.filter((snapshot) => snapshot.project?.health === 'green').length,
  active: props.snapshots.reduce((sum, snapshot) => sum
    + taskCount(snapshot, 'in-progress') + taskCount(snapshot, 'in-review'), 0),
  blocked: props.snapshots.reduce((sum, snapshot) => sum + blockerCount(snapshot), 0),
}))

const openFolder = async (snapshot: ProjectSnapshot) => {
  opening.value = snapshot.registration.id
  feedback.value = null
  try {
    await projects.openFolder(snapshot.registration.id)
    feedback.value = { kind: 'success', text: `已在文件管理器中打开 ${projectName(snapshot)}` }
  } catch (error) {
    feedback.value = { kind: 'error', text: `无法打开项目文件夹：${String(error)}` }
  } finally {
    opening.value = null
  }
}
</script>

<template>
  <section class="projects-view" aria-labelledby="projects-view-title">
    <header class="projects-heading">
      <div>
        <span class="eyebrow">Repository overview</span>
        <h1 id="projects-view-title">项目一览</h1>
        <p>集中查看所有本地项目的协议健康、任务流转、阻塞和诊断。</p>
      </div>
      <div class="project-summary" aria-label="项目汇总">
        <div><span>项目</span><strong>{{ summary.projects }}</strong></div>
        <div><span>健康</span><strong class="positive">{{ summary.healthy }}</strong></div>
        <div><span>进行中</span><strong>{{ summary.active }}</strong></div>
        <div><span>阻塞</span><strong :class="{ negative: summary.blocked > 0 }">{{ summary.blocked }}</strong></div>
      </div>
    </header>

    <p v-if="feedback" :class="['projects-feedback', feedback.kind]" :role="feedback.kind === 'error' ? 'alert' : 'status'">{{ feedback.text }}</p>

    <div v-if="rows.length" class="project-card-grid" aria-label="项目卡片列表">
      <article v-for="snapshot in rows" :key="snapshot.registration.id" class="project-overview-card">
        <header>
          <div class="project-card-title">
            <span :class="['project-health-label', snapshot.project?.health ?? 'unknown']"><i/>{{ healthLabel(snapshot.project?.health) }}</span>
            <h2>{{ projectName(snapshot) }}</h2>
            <code :title="snapshot.registration.path">{{ snapshot.registration.path }}</code>
          </div>
          <UiIcon name="folder" :size="24"/>
        </header>

        <div class="project-state-grid" aria-label="任务状态分布">
          <div v-for="state in states" :key="state.key" :title="state.label"><strong>{{ taskCount(snapshot, state.key) }}</strong><span>{{ state.short }}</span></div>
        </div>

        <div class="project-signals">
          <span :class="{ danger: blockerCount(snapshot) > 0 }"><UiIcon name="alert" :size="14"/>阻塞 {{ blockerCount(snapshot) }}</span>
          <span :class="{ warning: snapshot.diagnostics.length > 0 }"><UiIcon name="diagnostic" :size="14"/>诊断 {{ snapshot.diagnostics.length }}</span>
          <span>任务总数 {{ snapshot.tasks.length }}</span>
        </div>

        <footer>
          <button class="button secondary project-folder" :disabled="opening === snapshot.registration.id" @click="openFolder(snapshot)"><UiIcon name="folder" :size="15"/>{{ opening === snapshot.registration.id ? '正在打开…' : '打开文件夹' }}</button>
          <button class="button primary project-open" @click="$emit('open', snapshot.registration.id)">进入看板<UiIcon name="chevron" :size="15"/></button>
        </footer>
      </article>
    </div>
    <div v-else class="projects-no-results">没有匹配的项目</div>
  </section>
</template>
