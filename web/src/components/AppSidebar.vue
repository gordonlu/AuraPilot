<script setup lang="ts">
import type { ProjectSnapshot } from '../types/protocol'
import brandLogo from '../assets/aurapilot-logo.webp'
import brandMark from '../assets/aurapilot-mark.webp'
import UiIcon from './UiIcon.vue'

defineProps<{
  snapshots: ProjectSnapshot[]
  activeProject: string
  activeView: 'board' | 'blocked'
  theme: 'dark' | 'light'
  diagnosticCount: number
}>()

defineEmits<{
  project: [id: string]
  view: [view: 'board' | 'blocked']
  add: []
  theme: []
  diagnostics: []
  profiles: []
}>()
</script>

<template>
  <aside class="sidebar">
    <div class="brand-lockup">
      <img :src="brandLogo" alt="AuraPilot" class="brand-logo brand-logo-full" />
      <img :src="brandMark" alt="AuraPilot" class="brand-logo brand-logo-mark" />
    </div>

    <div class="sidebar-section-head">
      <span>项目</span>
      <button class="icon-button" aria-label="添加项目" title="添加项目" @click="$emit('add')">
        <UiIcon name="plus" :size="16" />
      </button>
    </div>
    <nav class="project-list" aria-label="项目筛选">
      <button :class="['nav-row', { active: activeProject === 'all' }]" @click="$emit('project', 'all')">
        <UiIcon name="folder"/><span>所有项目</span><b>{{ snapshots.length }}</b>
      </button>
      <button
        v-for="snapshot in snapshots"
        :key="snapshot.registration.id"
        :class="['nav-row', { active: activeProject === snapshot.registration.id }]"
        @click="$emit('project', snapshot.registration.id)"
      >
        <span :class="['health', snapshot.project?.health ?? 'unknown']" />
        <span class="truncate">{{ snapshot.project?.name ?? snapshot.registration.path.split('/').at(-1) }}</span>
        <b>{{ snapshot.tasks.length }}</b>
      </button>
    </nav>

    <nav class="primary-nav" aria-label="主导航">
      <button :class="['nav-row', { active: activeView === 'board' }]" @click="$emit('view', 'board')">
        <UiIcon name="board"/><span>看板</span>
      </button>
      <button :class="['nav-row', { active: activeView === 'blocked' }]" @click="$emit('view', 'blocked')">
        <UiIcon name="alert"/><span>阻塞聚焦</span>
        <b class="danger-count">{{ snapshots.flatMap((item) => item.tasks).filter((task) => task.document.blockers.length).length }}</b>
      </button>
    </nav>

    <div class="sidebar-footer">
      <button class="footer-control" @click="$emit('profiles')">
        <UiIcon name="terminal"/><span>Agent Profiles</span>
      </button>
      <button class="footer-control" @click="$emit('theme')">
        <UiIcon :name="theme === 'dark' ? 'moon' : 'sun'"/><span>{{ theme === 'dark' ? '暗色模式' : '浅色模式' }}</span>
      </button>
      <button class="footer-control" @click="$emit('diagnostics')">
        <UiIcon name="diagnostic"/><span>诊断面板</span><i :class="{ warning: diagnosticCount }" />
      </button>
    </div>
  </aside>
</template>
