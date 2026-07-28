<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { useAgentsStore } from '../stores/agents'
import type { AgentLaunchProfile, AgentProfileEntry, ProjectSnapshot } from '../types/protocol'
import UiIcon from './UiIcon.vue'

const props = defineProps<{ projects: ProjectSnapshot[] }>()
defineEmits<{ close: [] }>()
const agents = useAgentsStore()
const editing = ref(false)
const busy = ref(false)
const message = ref('')
const error = ref('')
const form = reactive({
  id: '', display_name: '', executable: '', args: '{prompt}', detect_commands: '',
  launch_mode: 'interactive_terminal' as AgentLaunchProfile['launch_mode'],
  prompt_transport: 'argument' as AgentLaunchProfile['prompt_transport'],
  fixed_path: '', show_terminal: true,
})

onMounted(() => agents.load())
const reset = () => {
  Object.assign(form, { id: '', display_name: '', executable: '', args: '{prompt}', detect_commands: '', launch_mode: 'interactive_terminal', prompt_transport: 'argument', fixed_path: '', show_terminal: true })
  editing.value = true; error.value = ''; message.value = ''
}
const edit = (entry: AgentProfileEntry) => {
  if (entry.built_in) return
  const profile = entry.profile
  Object.assign(form, {
    id: profile.id, display_name: profile.display_name, executable: profile.executable,
    args: profile.args.join('\n'), detect_commands: profile.detect_commands.join('\n'),
    launch_mode: profile.launch_mode, prompt_transport: profile.prompt_transport,
    fixed_path: profile.working_directory.kind === 'fixed_path' ? profile.working_directory.path : '',
    show_terminal: profile.show_terminal,
  })
  editing.value = true; error.value = ''; message.value = ''
}
const save = async () => {
  busy.value = true; error.value = ''
  try {
    await agents.save({
      id: form.id.trim(), display_name: form.display_name.trim(), executable: form.executable.trim(),
      args: form.args.split('\n').map((item) => item.trim()).filter(Boolean),
      detect_commands: form.detect_commands.split('\n').map((item) => item.trim()).filter(Boolean),
      working_directory: form.fixed_path.trim()
        ? { kind: 'fixed_path', path: form.fixed_path.trim() }
        : { kind: 'repository' },
      launch_mode: form.launch_mode, prompt_transport: form.prompt_transport,
      show_terminal: form.show_terminal,
    })
    editing.value = false; message.value = 'Profile 已保存'
  } catch (caught) { error.value = String(caught) } finally { busy.value = false }
}
const remove = async (id: string) => {
  busy.value = true; error.value = ''
  try { await agents.remove(id); message.value = 'Profile 已删除' }
  catch (caught) { error.value = String(caught) } finally { busy.value = false }
}
const testProfile = async (id: string) => {
  const project = props.projects[0]
  if (!project) { error.value = '请先注册项目再测试 Profile'; return }
  busy.value = true; error.value = ''
  try { message.value = (await agents.test(project.registration.id, id)).message }
  catch (caught) { error.value = String(caught) } finally { busy.value = false }
}
</script>

<template>
  <div class="modal-backdrop" @mousedown.self="$emit('close')">
    <section class="task-modal profiles-modal" role="dialog" aria-modal="true" aria-label="Agent Profiles">
      <header><div><span class="modal-mark"><UiIcon name="terminal"/></span><div><h2>Agent Profiles</h2><p>内置配置与本地自定义启动器</p></div></div><button class="icon-button" aria-label="关闭" @click="$emit('close')"><UiIcon name="x"/></button></header>
      <div class="modal-body profile-layout">
        <div class="profile-list">
          <article v-for="entry in agents.profiles" :key="entry.profile.id" class="profile-row">
            <span :class="['availability-dot', { available: entry.availability.available }]"/>
            <div><strong>{{ entry.profile.display_name }}</strong><code>{{ entry.profile.executable || 'clipboard' }}</code></div>
            <span class="profile-kind">{{ entry.built_in ? '内置' : '自定义' }}</span>
            <button class="button secondary" :disabled="busy" @click="testProfile(entry.profile.id)">测试</button>
            <button v-if="!entry.built_in" class="icon-button" title="编辑" @click="edit(entry)"><UiIcon name="edit" :size="15"/></button>
            <button v-if="!entry.built_in" class="icon-button danger-text" title="删除" @click="remove(entry.profile.id)"><UiIcon name="trash" :size="15"/></button>
          </article>
          <button class="button secondary full-width" @click="reset"><UiIcon name="plus" :size="15"/>新增自定义 Profile</button>
        </div>
        <form v-if="editing" class="profile-form" @submit.prevent="save">
          <h3>自定义 Profile</h3>
          <div class="form-grid">
            <label class="field"><span>ID</span><input v-model="form.id" required pattern="[A-Za-z0-9._-]+" placeholder="my-agent"/></label>
            <label class="field"><span>名称</span><input v-model="form.display_name" required placeholder="My Agent"/></label>
            <label class="field full"><span>Executable</span><input v-model="form.executable" required placeholder="my-agent"/></label>
            <label class="field full"><span>参数（每行一个）</span><textarea v-model="form.args" placeholder="--prompt&#10;{prompt}"/></label>
            <label class="field"><span>启动方式</span><select v-model="form.launch_mode"><option value="interactive_terminal">交互终端</option><option value="headless_process">后台进程</option><option value="clipboard_only">仅剪贴板</option></select></label>
            <label class="field"><span>Prompt 传输</span><select v-model="form.prompt_transport"><option value="argument">参数</option><option value="stdin">标准输入</option><option value="clipboard">剪贴板</option></select></label>
            <label class="field full"><span>固定工作目录 <small>留空使用仓库</small></span><input v-model="form.fixed_path" placeholder="/absolute/path"/></label>
            <label class="field full"><span>检测命令（每行一个）</span><textarea v-model="form.detect_commands" placeholder="my-agent"/></label>
          </div>
          <button class="button primary full-width" :disabled="busy">保存 Profile</button>
        </form>
        <p v-if="message" class="push-result success">{{ message }}</p><p v-if="error || agents.error" class="form-error">{{ error || agents.error }}</p>
      </div>
      <footer><button class="button secondary" @click="$emit('close')">完成</button></footer>
    </section>
  </div>
</template>
