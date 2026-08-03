<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  dialogueForPetEvent,
  interactionDialogue,
  PET_DIALOGUE_CONFIG,
  type PetDialogueContext,
} from './pets/dialogue'
import {
  WorldSkinController,
  type PetEventSignal,
  type WorldSkinRuntimeState,
  type WorldSkinViewport,
} from './runtime'
import type { WorldSkin } from './worldSkin'

const props = defineProps<{
  skin: WorldSkin
  event?: PetEventSignal | null
  context?: PetDialogueContext
}>()
const emit = defineEmits<{
  state: [state: WorldSkinRuntimeState]
}>()

const container = ref<HTMLElement | null>(null)
const host = ref<HTMLElement | null>(null)
const state = ref<WorldSkinRuntimeState>({ skin: props.skin, status: 'idle', error: null })
const retryKey = ref(0)
const dialogue = ref<string | null>(null)
let dialogueTimer: number | null = null
let interactionIndex = 0
let resizeObserver: ResizeObserver | null = null
let scrollContainer: HTMLElement | null = null

const controller = new WorldSkinController({
  loadRuntime: async (skin) => {
    if (skin === 'seascape') return import('./seascape/runtime')
    throw new Error(`未注册世界皮肤：${skin}`)
  },
  onStateChange: (next) => {
    state.value = next
    emit('state', next)
  },
})

const viewport = (element: HTMLElement): WorldSkinViewport => ({
  width: Math.max(1, element.clientWidth),
  height: Math.max(1, element.clientHeight),
  devicePixelRatio: window.devicePixelRatio || 1,
})

const activate = async () => {
  await nextTick()
  if (!container.value) return
  await controller.activate(props.skin, container.value)
  if (state.value.status === 'ready' && container.value) {
    controller.resize(viewport(container.value))
    if (document.hidden) controller.pause('document-hidden')
  }
}

const hideDialogue = () => {
  dialogue.value = null
  if (dialogueTimer !== null) window.clearTimeout(dialogueTimer)
  dialogueTimer = null
}

const showDialogue = (message: string) => {
  if (props.skin !== 'seascape') return
  hideDialogue()
  dialogue.value = message
  dialogueTimer = window.setTimeout(hideDialogue, PET_DIALOGUE_CONFIG.visibleDurationMs)
}

const interactWithPet = () => {
  controller.dispatch({ type: 'pet-interact' })
  showDialogue(interactionDialogue(interactionIndex++, props.context))
}

const syncHostToViewport = () => {
  if (!host.value || !scrollContainer) return
  host.value.style.transform = `translate3d(${scrollContainer.scrollLeft}px, ${scrollContainer.scrollTop}px, 0)`
}

const onVisibilityChange = () => {
  if (document.hidden) controller.pause('document-hidden')
  else controller.resume()
}

watch(() => [props.skin, retryKey.value] as const, activate)
watch(() => props.event?.sequence, () => {
  if (!props.event) return
  controller.dispatch({ type: 'pet-state', event: props.event.event })
  showDialogue(dialogueForPetEvent(props.event.event, props.event.sequence))
})
watch(() => props.skin, (skin) => {
  if (skin !== 'seascape') hideDialogue()
})

onMounted(() => {
  scrollContainer = host.value?.parentElement ?? null
  scrollContainer?.addEventListener('scroll', syncHostToViewport, { passive: true })
  syncHostToViewport()
  if (container.value && typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver(() => {
      if (container.value) controller.resize(viewport(container.value))
    })
    resizeObserver.observe(container.value)
  }
  document.addEventListener('visibilitychange', onVisibilityChange)
  void activate()
})

onBeforeUnmount(() => {
  hideDialogue()
  scrollContainer?.removeEventListener('scroll', syncHostToViewport)
  scrollContainer = null
  resizeObserver?.disconnect()
  document.removeEventListener('visibilitychange', onVisibilityChange)
  controller.dispose()
})
</script>

<template>
  <div
    ref="host"
    class="world-skin-host"
    :data-skin="skin"
    :data-status="state.status"
    aria-label="世界皮肤运行时"
  >
    <div ref="container" class="world-skin-stage">
      <div
        v-if="skin === 'seascape' && state.status === 'ready' && dialogue"
        class="world-skin-pet-dialogue"
        role="status"
        aria-live="polite"
      >
        {{ dialogue }}
      </div>
      <button
        v-if="skin === 'seascape' && state.status === 'ready'"
        type="button"
        class="world-skin-pet-hitbox"
        aria-label="和星贝打招呼"
        title="和星贝打招呼"
        @click="interactWithPet"
      />
    </div>
    <div v-if="state.status === 'loading'" class="world-skin-feedback loading" role="status">
      <span />正在加载海岸世界…
    </div>
    <div v-else-if="state.status === 'error'" class="world-skin-feedback error" role="alert">
      <strong>海岸世界启动失败</strong>
      <small>{{ state.error }}</small>
      <button type="button" @click="retryKey++">重试</button>
    </div>
    <div v-else-if="skin === 'seascape'" class="world-skin-beta">
      海岸世界运行时 · BETA
    </div>
  </div>
</template>
