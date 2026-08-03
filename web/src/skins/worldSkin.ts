export type WorldSkin = 'classic' | 'seascape'

export const DEFAULT_WORLD_SKIN: WorldSkin = 'classic'
export const WORLD_SKIN_STORAGE_KEY = 'aurapilot-world-skin'
export const WORLD_SKIN_ORDER: WorldSkin[] = ['classic', 'seascape']

export const WORLD_SKIN_CONFIG = Object.freeze({
  loadTimeoutMs: 8_000,
})

export const WORLD_SKIN_PRESENTATION: Record<WorldSkin, {
  label: string
  actionLabel: string
  description: string
  motionLabel: string
  beta: boolean
}> = {
  classic: {
    label: '经典界面',
    actionLabel: '选择世界皮肤',
    description: '清晰、安静的标准任务工作台。',
    motionLabel: '静态界面',
    beta: false,
  },
  seascape: {
    label: '海岸世界',
    actionLabel: '选择世界皮肤',
    description: '潮汐、浪花与星贝陪你推进任务。',
    motionLabel: '动态世界',
    beta: true,
  },
}

export const resolveWorldSkin = (saved: string | null): WorldSkin =>
  saved === 'seascape' ? saved : DEFAULT_WORLD_SKIN

export const nextWorldSkin = (current: WorldSkin): WorldSkin =>
  WORLD_SKIN_ORDER[(WORLD_SKIN_ORDER.indexOf(current) + 1) % WORLD_SKIN_ORDER.length]
