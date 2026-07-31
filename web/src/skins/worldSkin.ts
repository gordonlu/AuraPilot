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
  beta: boolean
}> = {
  classic: {
    label: '经典界面',
    actionLabel: '切换到海岸世界',
    beta: false,
  },
  seascape: {
    label: '海岸世界',
    actionLabel: '切换到经典界面',
    beta: true,
  },
}

export const resolveWorldSkin = (saved: string | null): WorldSkin =>
  saved === 'seascape' ? saved : DEFAULT_WORLD_SKIN

export const nextWorldSkin = (current: WorldSkin): WorldSkin =>
  WORLD_SKIN_ORDER[(WORLD_SKIN_ORDER.indexOf(current) + 1) % WORLD_SKIN_ORDER.length]
