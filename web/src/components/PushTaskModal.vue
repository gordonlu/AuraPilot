<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useAgentsStore } from '../stores/agents'
import type { LocatedTask, PointerPrompt, ProjectSnapshot, PushOutcome } from '../types/protocol'
import UiIcon from './UiIcon.vue'

const props = defineProps<{ task: LocatedTask; project: ProjectSnapshot }>()
defineEmits<{ close: [] }>()
const agents = useAgentsStore()
const selected = ref('')
const preview = ref<PointerPrompt | null>(null)
const outcome = ref<PushOutcome | null>(null)
const busy = ref(false)
const error = ref<string | null>(null)
const selectedEntry = computed(() => agents.profiles.find((entry) => entry.profile.id === selected.value))

onMounted(async () => {
  try {
    await agents.load()
    selected.value = agents.profiles.find((entry) => entry.profile.id === props.project.registration.last_profile_id)?.profile.id
      ?? agents.profiles.find((entry) => entry.availability.available && entry.profile.id !== 'clipboard-only')?.profile.id
      ?? agents.profiles.find((entry) => entry.profile.id === 'clipboard-only')?.profile.id
      ?? ''
    if (props.task.document.id) preview.value = await agents.preview(props.project.registration.id, props.task.document.id)
  } catch (caught) {
    error.value = `无法准备 Push：${String(caught)}`
  }
})

const push = async () => {
  if (!props.task.document.id || !selected.value) return
  busy.value = true
  error.value = null
  try {
    outcome.value = await agents.push(props.project.registration.id, props.task.document.id, selected.value)
    props.project.registration.last_profile_id = selected.value
  } catch (caught) {
    error.value = String(caught)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="modal-backdrop" @mousedown.self="$emit('close')">
    <section class="task-modal push-modal" role="dialog" aria-modal="true" aria-label="Push 任务">
      <header>
        <div><span class="modal-mark"><UiIcon name="send"/></span><div><h2>Push {{ task.document.id }}</h2><p>仅传递任务入口，不修改任务状态</p></div></div>
        <button class="icon-button" aria-label="关闭" @click="$emit('close')"><UiIcon name="x"/></button>
      </header>
      <div class="modal-body">
        <p class="push-notice">AuraPilot 不管理多个 Agent，只允许保存多个启动方式。你可以顺序多次 Push；每次只手动选择一个 Profile。</p>
        <div v-if="agents.loading" class="inline-loading">正在检测 Agent…</div>
        <div v-else class="agent-grid">
          <button
            v-for="entry in agents.profiles" :key="entry.profile.id"
            :class="['agent-option', { selected: selected === entry.profile.id }]"
            @click="selected = entry.profile.id"
          >
            <span :class="['availability-dot', { available: entry.availability.available }]"/>
            <strong>{{ entry.profile.display_name }}</strong>
            <small>{{ entry.availability.available ? entry.availability.detail : '未检测到，可使用剪贴板兜底' }}</small>
            <b v-if="entry.profile.id === 'opencode'">OpenCode</b>
          </button>
        </div>
        <section v-if="preview" class="prompt-preview">
          <div><strong>Pointer Prompt</strong><span>{{ preview.text.length }} 字符</span></div>
          <pre>{{ preview.text }}</pre>
        </section>
        <p v-if="outcome" :class="['push-result', outcome.attempt.status === 'failed_to_start' ? 'warning' : 'success']">{{ outcome.message }}</p>
        <p v-if="error || agents.error" class="form-error">{{ error || agents.error }}</p>
      </div>
      <footer>
        <span class="push-safety">任务仍保持 {{ task.state }}</span>
        <button class="button secondary" @click="$emit('close')">关闭</button>
        <button class="button primary" :disabled="busy || !selectedEntry" @click="push">
          <UiIcon name="send" :size="15"/>{{ busy ? '正在启动…' : `Push 给 ${selectedEntry?.profile.display_name ?? 'Agent'}` }}
        </button>
      </footer>
    </section>
  </div>
</template>
