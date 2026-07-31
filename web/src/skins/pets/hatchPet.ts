import {
  PET_MANIFEST_FORMAT_VERSION,
  parsePetManifest,
  type PetAnimation,
  type PetEventId,
  type PetManifest,
} from './manifest'

export const HATCH_PET_ATLAS = Object.freeze({
  width: 1536,
  height: 1872,
  columns: 8,
  rows: 9,
  cellWidth: 192,
  cellHeight: 208,
})

export const HATCH_PET_ANIMATIONS = Object.freeze({
  idle: {
    row: 0,
    frames: [0, 1, 2, 3, 4, 5],
    durationsMs: [280, 110, 110, 140, 140, 320],
    loop: true,
  },
  'running-right': {
    row: 1,
    frames: [0, 1, 2, 3, 4, 5, 6, 7],
    durationsMs: [120, 120, 120, 120, 120, 120, 120, 220],
    loop: true,
  },
  'running-left': {
    row: 2,
    frames: [0, 1, 2, 3, 4, 5, 6, 7],
    durationsMs: [120, 120, 120, 120, 120, 120, 120, 220],
    loop: true,
  },
  waving: {
    row: 3,
    frames: [0, 1, 2, 3],
    durationsMs: [140, 140, 140, 280],
    loop: false,
  },
  jumping: {
    row: 4,
    frames: [0, 1, 2, 3, 4],
    durationsMs: [140, 140, 140, 140, 280],
    loop: false,
  },
  failed: {
    row: 5,
    frames: [0, 1, 2, 3, 4, 5, 6, 7],
    durationsMs: [140, 140, 140, 140, 140, 140, 140, 240],
    loop: false,
  },
  waiting: {
    row: 6,
    frames: [0, 1, 2, 3, 4, 5],
    durationsMs: [150, 150, 150, 150, 150, 260],
    loop: true,
  },
  running: {
    row: 7,
    frames: [0, 1, 2, 3, 4, 5],
    durationsMs: [120, 120, 120, 120, 120, 220],
    loop: true,
  },
  review: {
    row: 8,
    frames: [0, 1, 2, 3, 4, 5],
    durationsMs: [150, 150, 150, 150, 150, 280],
    loop: true,
  },
} satisfies Record<string, PetAnimation>)

export const HATCH_PET_EVENT_BINDINGS = Object.freeze({
  'task-created': 'waving',
  'task-blocked': 'waiting',
  'task-done': 'jumping',
  'task-review': 'review',
  'push-started': 'running',
  'push-succeeded': 'jumping',
  'push-failed': 'failed',
  'sync-failed': 'failed',
} satisfies Record<PetEventId, keyof typeof HATCH_PET_ANIMATIONS>)

interface HatchPetMetadata {
  id?: unknown
  displayName?: unknown
  description?: unknown
  spritesheetPath?: unknown
}

const metadataRecord = (input: unknown): HatchPetMetadata =>
  typeof input === 'object' && input !== null && !Array.isArray(input)
    ? input as HatchPetMetadata
    : {}

export const createHatchPetManifest = (input: unknown): PetManifest => {
  const metadata = metadataRecord(input)
  const displayName = typeof metadata.displayName === 'string'
    ? metadata.displayName
    : ''

  return parsePetManifest({
    formatVersion: PET_MANIFEST_FORMAT_VERSION,
    id: metadata.id,
    displayName: metadata.displayName,
    description: metadata.description,
    atlas: {
      src: metadata.spritesheetPath,
      ...HATCH_PET_ATLAS,
    },
    animations: HATCH_PET_ANIMATIONS,
    eventBindings: HATCH_PET_EVENT_BINDINGS,
    interactions: [{
      id: 'pet-tap',
      label: displayName ? `与${displayName}打招呼` : '与宠物打招呼',
      bounds: { x: 0, y: 0, width: 1, height: 1 },
      animation: 'waving',
    }],
  })
}
