<script setup lang="ts">
import { computed } from 'vue'
import type { WorldSkinRuntimeState } from '../skins/runtime'
import {
  WORLD_SKIN_ORDER,
  WORLD_SKIN_PRESENTATION,
  type WorldSkin,
} from '../skins/worldSkin'
import UiIcon from './UiIcon.vue'

const props = defineProps<{
  current: WorldSkin
  runtimeState: WorldSkinRuntimeState
  error?: string | null
}>()

defineEmits<{
  close: []
  select: [skin: WorldSkin]
}>()

const statusFor = computed(() => (skin: WorldSkin): string => {
  if (skin !== props.current) {
    if (skin === 'seascape' && props.error) return '启动失败'
    return '可选择'
  }
  if (skin === 'classic') return '当前使用'
  if (props.runtimeState.skin !== skin) return '正在准备'
  if (props.runtimeState.status === 'loading') return '正在启动'
  if (props.runtimeState.status === 'ready') return '运行中'
  if (props.runtimeState.status === 'error') return '启动失败'
  return '正在准备'
})
</script>

<template>
  <div class="modal-backdrop" @mousedown.self="$emit('close')">
    <section
      class="task-modal world-skin-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="world-skin-title"
      aria-describedby="world-skin-description"
    >
      <header>
        <div>
          <span class="modal-mark"><UiIcon name="compass"/></span>
          <div>
            <h2 id="world-skin-title">选择世界皮肤</h2>
            <p id="world-skin-description">皮肤只改变工作空间的视觉与互动，不改变任务协议</p>
          </div>
        </div>
        <button class="icon-button" type="button" aria-label="关闭世界皮肤选择器" @click="$emit('close')">
          <UiIcon name="x"/>
        </button>
      </header>

      <div class="modal-body">
        <div class="world-skin-grid" role="radiogroup" aria-label="可用世界皮肤">
          <button
            v-for="skin in WORLD_SKIN_ORDER"
            :key="skin"
            type="button"
            role="radio"
            :aria-checked="current === skin"
            :class="['world-skin-option', skin, { selected: current === skin }]"
            @click="$emit('select', skin)"
          >
            <span class="world-skin-preview" aria-hidden="true"><i/><b/><em/></span>
            <span class="world-skin-copy">
              <span class="world-skin-option-title">
                <strong>{{ WORLD_SKIN_PRESENTATION[skin].label }}</strong>
                <small v-if="WORLD_SKIN_PRESENTATION[skin].beta">BETA</small>
              </span>
              <span>{{ WORLD_SKIN_PRESENTATION[skin].description }}</span>
            </span>
            <span class="world-skin-option-footer">
              <span><UiIcon :name="skin === 'seascape' ? 'sun' : 'board'" :size="14"/>{{ WORLD_SKIN_PRESENTATION[skin].motionLabel }}</span>
              <b :class="{ active: current === skin, error: statusFor(skin) === '启动失败' }">
                {{ statusFor(skin) }}
              </b>
            </span>
          </button>
        </div>

        <p
          v-if="error"
          class="world-skin-runtime-error"
          role="alert"
        >
          <UiIcon name="alert" :size="15"/>
          <span>{{ error }}。可以重新选择海岸世界重试，或继续使用经典界面。</span>
        </p>
      </div>

      <footer>
        <span class="push-safety">动态世界仍处于 Beta，启动失败时会自动回退</span>
        <button class="button primary" type="button" @click="$emit('close')">完成</button>
      </footer>
    </section>
  </div>
</template>
