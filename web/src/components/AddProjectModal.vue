<script setup lang="ts">
import UiIcon from './UiIcon.vue'
const props = defineProps<{
  path: string
  busy?: boolean
  selecting?: boolean
  error?: string | null
  canInitialize?: boolean
}>()
const emit = defineEmits<{
  close: []
  add: [path: string]
  browse: []
  initialize: [path: string]
  'update:path': [path: string]
}>()
</script>

<template>
  <div class="modal-backdrop" @mousedown.self="$emit('close')">
    <form
      class="small-modal add-project-modal" role="dialog" aria-modal="true"
      aria-labelledby="add-project-title" aria-describedby="add-project-description"
      @keydown.esc.prevent="$emit('close')"
      @submit.prevent="props.path.trim() && $emit('add', props.path.trim())"
    >
      <header><div><span class="modal-mark"><UiIcon name="folder"/></span><div><h2 id="add-project-title">接入本地项目</h2><p id="add-project-description">选择代码仓库；需要时可直接初始化 AuraPilot 协议</p></div></div><button type="button" class="icon-button" aria-label="关闭" @click="$emit('close')"><UiIcon name="x"/></button></header>
      <div class="modal-body">
        <label class="field">
          <span>项目目录</span>
          <span class="path-picker">
            <input
              :value="props.path" autofocus placeholder="选择目录，或粘贴仓库绝对路径"
              autocomplete="off" spellcheck="false"
              @input="$emit('update:path', ($event.target as HTMLInputElement).value)"
            />
            <button type="button" class="button secondary browse-button" :disabled="busy || selecting" @click="$emit('browse')">
              <UiIcon name="folder" :size="15"/>{{ selecting ? '选择中…' : '选择目录' }}
            </button>
          </span>
        </label>
        <p class="field-help">不会修改项目源码、Git 历史或全局 Agent 配置。</p>
        <div v-if="canInitialize" class="initialization-callout" role="status" aria-live="polite">
          <strong>这个项目还没有 AuraPilot 协议</strong>
          <p>可以创建 <code>.aurapilot/</code> 目录和基础协议文件，然后自动添加到项目列表。</p>
          <button type="button" class="button primary" :disabled="busy" @click="$emit('initialize', props.path.trim())">
            {{ busy ? '正在初始化…' : '初始化并添加' }}
          </button>
        </div>
        <p v-else-if="error" class="form-error" role="alert" aria-live="assertive">{{ error }}</p>
      </div>
      <footer>
        <button type="button" class="button secondary" @click="$emit('close')">取消</button>
        <button v-if="!canInitialize" class="button primary" :disabled="!props.path.trim() || busy || selecting">{{ busy ? '正在验证…' : '添加项目' }}</button>
      </footer>
    </form>
  </div>
</template>
