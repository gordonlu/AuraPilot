<script setup lang="ts">
import { ref } from 'vue'
import UiIcon from './UiIcon.vue'
defineProps<{ busy?: boolean; error?: string | null }>()
const emit = defineEmits<{ close: []; add: [path: string] }>()
const path = ref('')
</script>

<template>
  <div class="modal-backdrop" @mousedown.self="$emit('close')">
    <form class="small-modal" role="dialog" aria-modal="true" aria-label="添加已有项目" @submit.prevent="path.trim() && $emit('add', path.trim())">
      <header><div><span class="modal-mark"><UiIcon name="folder"/></span><div><h2>添加已有项目</h2><p>目录必须包含有效的 .aurapilot/ 协议</p></div></div><button type="button" class="icon-button" aria-label="关闭" @click="$emit('close')"><UiIcon name="x"/></button></header>
      <div class="modal-body"><label class="field"><span>仓库绝对路径</span><input v-model="path" autofocus placeholder="/home/user/code/my-project" /></label><p v-if="error" class="form-error">{{ error }}</p></div>
      <footer><button type="button" class="button secondary" @click="$emit('close')">取消</button><button class="button primary" :disabled="!path.trim() || busy">{{ busy ? '验证中…' : '添加项目' }}</button></footer>
    </form>
  </div>
</template>
