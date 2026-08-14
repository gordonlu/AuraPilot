<script setup lang="ts">
import { nextTick, onMounted, reactive, ref } from 'vue'
import { useProjectsStore } from '../stores/projects'
import type { ProjectSnapshot, RepairPlan } from '../types/protocol'
import UiIcon from './UiIcon.vue'

const props = defineProps<{ snapshots: ProjectSnapshot[]; focusPath?: string | null }>()
defineEmits<{ close: [] }>()

const projects = useProjectsStore()
const plans = ref<Record<string, RepairPlan[]>>({})
const loading = reactive<Record<string, boolean>>({})
const errors = reactive<Record<string, string | null>>({})
const notices = reactive<Record<string, string | null>>({})
const confirming = ref<string | null>(null)

const fixable = (plan: RepairPlan) => plan.action.type !== 'manual'
const planLabel = (plan: RepairPlan) => ({
  fill_protocol_fields: '补全协议字段', rename_file: '修正文件名', manual: '需要人工处理',
}[plan.kind])

const preview = async (projectId: string) => {
  if (loading[projectId]) return
  loading[projectId] = true
  errors[projectId] = null
  notices[projectId] = null
  confirming.value = null
  try {
    plans.value[projectId] = await projects.previewRepairs(projectId)
  } catch (error) {
    errors[projectId] = `生成修复方案失败：${String(error)}`
  } finally {
    loading[projectId] = false
  }
}

const apply = async (projectId: string, plan: RepairPlan) => {
  if (confirming.value !== plan.id) {
    confirming.value = plan.id
    return
  }
  loading[projectId] = true
  errors[projectId] = null
  try {
    const report = await projects.applyRepair(projectId, plan)
    notices[projectId] = `${report.applied.message}；诊断已刷新`
    plans.value[projectId] = await projects.previewRepairs(projectId)
  } catch (error) {
    errors[projectId] = `修复失败：${String(error)}`
  } finally {
    loading[projectId] = false
    confirming.value = null
  }
}
onMounted(async () => {
  if (!props.focusPath) return
  const snapshot = props.snapshots.find((item) => item.diagnostics.some((diagnostic) => diagnostic.path === props.focusPath))
  if (!snapshot) return
  await preview(snapshot.registration.id)
  await nextTick()
  document.querySelector(`[data-repair-path="${CSS.escape(props.focusPath)}"]`)?.scrollIntoView({ block: 'center' })
})
</script>

<template>
  <aside class="diagnostics-panel" aria-label="协议诊断">
    <header><div><UiIcon name="diagnostic"/><h2>协议诊断</h2></div><button class="icon-button" aria-label="关闭诊断" @click="$emit('close')"><UiIcon name="x"/></button></header>
    <div class="diagnostics-content">
      <template v-for="snapshot in snapshots" :key="snapshot.registration.id">
        <section v-if="snapshot.diagnostics.length" class="diagnostic-group">
          <h3>{{ snapshot.project?.name ?? snapshot.registration.path }}</h3>
          <article v-for="(item, index) in snapshot.diagnostics" :key="`${item.path}-${index}`" :class="['diagnostic-item', item.severity]">
            <div><strong>{{ item.code.replaceAll('_', ' ') }}</strong><span>{{ item.severity }}</span></div>
            <p>{{ item.message }}</p><code v-if="item.path">{{ item.path }}</code>
          </article>
          <div class="repair-toolbar">
            <button class="button secondary" :disabled="loading[snapshot.registration.id]" @click="preview(snapshot.registration.id)">
              <UiIcon name="diagnostic" :size="14"/>{{ loading[snapshot.registration.id] ? '正在检查…' : plans[snapshot.registration.id] ? '重新检查修复方案' : '查看修复方案' }}
            </button>
            <small>先预览、再确认；文件发生变化时会拒绝覆盖。</small>
          </div>
          <p v-if="errors[snapshot.registration.id]" class="repair-notice error" role="alert">{{ errors[snapshot.registration.id] }}</p>
          <p v-if="notices[snapshot.registration.id]" class="repair-notice ok" role="status">{{ notices[snapshot.registration.id] }}</p>
          <p v-if="plans[snapshot.registration.id] && !plans[snapshot.registration.id].length" class="repair-empty">当前没有任务文件修复项。</p>
          <article v-for="plan in plans[snapshot.registration.id]" :key="plan.id" :data-repair-path="plan.path" :class="['repair-card', { manual: !fixable(plan), focused: plan.path === focusPath }]">
            <header><strong>{{ planLabel(plan) }}</strong><b>{{ fixable(plan) ? '可确认修复' : '仅提示' }}</b></header>
            <h4>{{ plan.summary }}</h4>
            <code>{{ plan.path }}</code>
            <p>{{ plan.detail }}</p>
            <template v-if="plan.action.type === 'rewrite' || plan.action.type === 'rename_file'">
              <ul><li v-for="change in plan.action.changes" :key="change">{{ change }}</li></ul>
              <p v-if="plan.action.type === 'rename_file'">目标：<code>{{ plan.action.target }}</code></p>
              <details><summary>查看修改后的 YAML</summary><pre>{{ plan.action.new_content }}</pre></details>
            </template>
            <template v-else>
              <p class="repair-reason">{{ plan.action.reason }}</p>
              <p>建议：{{ plan.action.suggestion }}</p>
            </template>
            <footer v-if="fixable(plan)">
              <button v-if="confirming === plan.id" class="button secondary" @click="confirming = null">取消</button>
              <button class="button primary" :disabled="loading[snapshot.registration.id]" @click="apply(snapshot.registration.id, plan)">
                {{ confirming === plan.id ? '确认执行此修复' : '应用修复' }}
              </button>
            </footer>
          </article>
        </section>
      </template>
      <div v-if="!snapshots.some((item) => item.diagnostics.length)" class="panel-empty"><span>✓</span><h3>没有发现协议问题</h3><p>所有已扫描任务均可正常读取。</p></div>
    </div>
  </aside>
</template>
