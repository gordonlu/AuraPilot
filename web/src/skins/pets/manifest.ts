export const PET_MANIFEST_FORMAT_VERSION = 1 as const

export const PET_MANIFEST_LIMITS = Object.freeze({
  maxAtlasDimension: 8192,
  maxCellDimension: 1024,
  maxAnimations: 64,
  maxFramesPerAnimation: 64,
  minFrameDurationMs: 16,
  maxFrameDurationMs: 60_000,
  maxInteractions: 32,
})

export const PET_EVENT_IDS = [
  'task-created',
  'task-blocked',
  'task-done',
  'task-review',
  'push-started',
  'push-succeeded',
  'push-failed',
  'sync-failed',
] as const

export type PetEventId = typeof PET_EVENT_IDS[number]

export interface PetAtlas {
  src: string
  width: number
  height: number
  columns: number
  rows: number
  cellWidth: number
  cellHeight: number
}

export interface PetAnimation {
  row: number
  frames: number[]
  durationsMs: number[]
  loop: boolean
}

export interface PetInteractionBounds {
  x: number
  y: number
  width: number
  height: number
}

export interface PetInteraction {
  id: string
  label: string
  bounds: PetInteractionBounds
  animation: string
}

export interface PetManifest {
  formatVersion: typeof PET_MANIFEST_FORMAT_VERSION
  id: string
  displayName: string
  description?: string
  atlas: PetAtlas
  animations: Record<string, PetAnimation>
  eventBindings: Partial<Record<PetEventId, string>>
  interactions: PetInteraction[]
}

export class PetManifestValidationError extends Error {
  constructor(readonly issues: readonly string[]) {
    super(`invalid pet manifest:\n- ${issues.join('\n- ')}`)
    this.name = 'PetManifestValidationError'
  }
}

const IDENTIFIER_PATTERN = /^[a-z][a-z0-9-]{0,63}$/
const SUPPORTED_ATLAS_PATTERN = /\.(png|webp)$/i
const EVENT_IDS = new Set<string>(PET_EVENT_IDS)

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const readString = (
  value: unknown,
  path: string,
  issues: string[],
  options: { optional?: boolean; identifier?: boolean } = {},
): string | undefined => {
  if (options.optional && value === undefined) return undefined
  if (typeof value !== 'string' || !value.trim()) {
    issues.push(`${path} must be a non-empty string`)
    return undefined
  }
  const result = value.trim()
  if (options.identifier && !IDENTIFIER_PATTERN.test(result)) {
    issues.push(`${path} must use lowercase kebab-case and contain at most 64 characters`)
  }
  return result
}

const readInteger = (
  value: unknown,
  path: string,
  issues: string[],
  minimum: number,
  maximum: number,
): number => {
  if (!Number.isInteger(value) || (value as number) < minimum || (value as number) > maximum) {
    issues.push(`${path} must be an integer between ${minimum} and ${maximum}`)
    return minimum
  }
  return value as number
}

const readUnitNumber = (value: unknown, path: string, issues: string[]): number => {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value > 1) {
    issues.push(`${path} must be a finite number between 0 and 1`)
    return 0
  }
  return value
}

const readAtlasSource = (value: unknown, issues: string[]): string => {
  const source = readString(value, 'atlas.src', issues) ?? ''
  const hasUnsafeSegment = source.split('/').some((segment) => segment === '.' || segment === '..')
  const isAbsolute = source.startsWith('/') || source.startsWith('\\') || /^[a-z]:/i.test(source)
  const hasScheme = /^[a-z][a-z0-9+.-]*:/i.test(source)

  if (
    source.includes('\\')
    || isAbsolute
    || hasScheme
    || hasUnsafeSegment
    || !SUPPORTED_ATLAS_PATTERN.test(source)
  ) {
    issues.push('atlas.src must be a safe relative PNG or WebP path inside the pet package')
  }
  return source
}

const readAtlas = (value: unknown, issues: string[]): PetAtlas => {
  const atlas = isRecord(value) ? value : {}
  if (!isRecord(value)) issues.push('atlas must be an object')

  const width = readInteger(
    atlas.width,
    'atlas.width',
    issues,
    1,
    PET_MANIFEST_LIMITS.maxAtlasDimension,
  )
  const height = readInteger(
    atlas.height,
    'atlas.height',
    issues,
    1,
    PET_MANIFEST_LIMITS.maxAtlasDimension,
  )
  const columns = readInteger(atlas.columns, 'atlas.columns', issues, 1, 64)
  const rows = readInteger(atlas.rows, 'atlas.rows', issues, 1, 64)
  const cellWidth = readInteger(
    atlas.cellWidth,
    'atlas.cellWidth',
    issues,
    1,
    PET_MANIFEST_LIMITS.maxCellDimension,
  )
  const cellHeight = readInteger(
    atlas.cellHeight,
    'atlas.cellHeight',
    issues,
    1,
    PET_MANIFEST_LIMITS.maxCellDimension,
  )

  if (width !== columns * cellWidth) {
    issues.push('atlas.width must equal atlas.columns × atlas.cellWidth')
  }
  if (height !== rows * cellHeight) {
    issues.push('atlas.height must equal atlas.rows × atlas.cellHeight')
  }

  return {
    src: readAtlasSource(atlas.src, issues),
    width,
    height,
    columns,
    rows,
    cellWidth,
    cellHeight,
  }
}

const readAnimations = (
  value: unknown,
  atlas: PetAtlas,
  issues: string[],
): Record<string, PetAnimation> => {
  if (!isRecord(value)) {
    issues.push('animations must be an object')
    return {}
  }

  const entries = Object.entries(value)
  if (!entries.length || entries.length > PET_MANIFEST_LIMITS.maxAnimations) {
    issues.push(`animations must contain between 1 and ${PET_MANIFEST_LIMITS.maxAnimations} entries`)
  }

  const animations: Record<string, PetAnimation> = {}
  for (const [name, candidate] of entries.slice(0, PET_MANIFEST_LIMITS.maxAnimations)) {
    if (!IDENTIFIER_PATTERN.test(name)) {
      issues.push(`animations.${name} must use a lowercase kebab-case key`)
      continue
    }
    if (!isRecord(candidate)) {
      issues.push(`animations.${name} must be an object`)
      continue
    }

    const row = readInteger(candidate.row, `animations.${name}.row`, issues, 0, atlas.rows - 1)
    const rawFrames = Array.isArray(candidate.frames) ? candidate.frames : []
    if (
      !Array.isArray(candidate.frames)
      || !rawFrames.length
      || rawFrames.length > PET_MANIFEST_LIMITS.maxFramesPerAnimation
    ) {
      issues.push(
        `animations.${name}.frames must contain between 1 and `
        + `${PET_MANIFEST_LIMITS.maxFramesPerAnimation} frame indexes`,
      )
    }
    const frames = rawFrames
      .slice(0, PET_MANIFEST_LIMITS.maxFramesPerAnimation)
      .map((frame, index) => readInteger(
        frame,
        `animations.${name}.frames[${index}]`,
        issues,
        0,
        atlas.columns - 1,
      ))

    const rawDurations = Array.isArray(candidate.durationsMs) ? candidate.durationsMs : []
    if (!Array.isArray(candidate.durationsMs) || rawDurations.length !== frames.length) {
      issues.push(`animations.${name}.durationsMs must have one duration for every frame`)
    }
    const durationsMs = rawDurations
      .slice(0, PET_MANIFEST_LIMITS.maxFramesPerAnimation)
      .map((duration, index) => readInteger(
        duration,
        `animations.${name}.durationsMs[${index}]`,
        issues,
        PET_MANIFEST_LIMITS.minFrameDurationMs,
        PET_MANIFEST_LIMITS.maxFrameDurationMs,
      ))

    if (typeof candidate.loop !== 'boolean') {
      issues.push(`animations.${name}.loop must be a boolean`)
    }
    animations[name] = { row, frames, durationsMs, loop: candidate.loop === true }
  }

  if (!animations.idle) issues.push('animations.idle is required')
  return animations
}

const readEventBindings = (
  value: unknown,
  animations: Record<string, PetAnimation>,
  issues: string[],
): Partial<Record<PetEventId, string>> => {
  if (value === undefined) return {}
  if (!isRecord(value)) {
    issues.push('eventBindings must be an object')
    return {}
  }

  const bindings: Partial<Record<PetEventId, string>> = {}
  for (const [event, candidate] of Object.entries(value)) {
    if (!EVENT_IDS.has(event)) {
      issues.push(`eventBindings.${event} is not a supported AuraPilot pet event`)
      continue
    }
    const animation = readString(candidate, `eventBindings.${event}`, issues)
    if (animation && !animations[animation]) {
      issues.push(`eventBindings.${event} references missing animation ${animation}`)
      continue
    }
    if (animation) bindings[event as PetEventId] = animation
  }
  return bindings
}

const readInteractions = (
  value: unknown,
  animations: Record<string, PetAnimation>,
  issues: string[],
): PetInteraction[] => {
  if (value === undefined) return []
  if (!Array.isArray(value)) {
    issues.push('interactions must be an array')
    return []
  }
  if (value.length > PET_MANIFEST_LIMITS.maxInteractions) {
    issues.push(`interactions must contain at most ${PET_MANIFEST_LIMITS.maxInteractions} entries`)
  }

  const seen = new Set<string>()
  return value.slice(0, PET_MANIFEST_LIMITS.maxInteractions).flatMap((candidate, index) => {
    const path = `interactions[${index}]`
    if (!isRecord(candidate)) {
      issues.push(`${path} must be an object`)
      return []
    }

    const id = readString(candidate.id, `${path}.id`, issues, { identifier: true }) ?? ''
    if (seen.has(id)) issues.push(`${path}.id duplicates interaction ${id}`)
    seen.add(id)
    const label = readString(candidate.label, `${path}.label`, issues) ?? ''
    const animation = readString(candidate.animation, `${path}.animation`, issues) ?? ''
    if (animation && !animations[animation]) {
      issues.push(`${path}.animation references missing animation ${animation}`)
    }

    const rawBounds = isRecord(candidate.bounds) ? candidate.bounds : {}
    if (!isRecord(candidate.bounds)) issues.push(`${path}.bounds must be an object`)
    const bounds = {
      x: readUnitNumber(rawBounds.x, `${path}.bounds.x`, issues),
      y: readUnitNumber(rawBounds.y, `${path}.bounds.y`, issues),
      width: readUnitNumber(rawBounds.width, `${path}.bounds.width`, issues),
      height: readUnitNumber(rawBounds.height, `${path}.bounds.height`, issues),
    }
    if (bounds.width === 0 || bounds.height === 0) {
      issues.push(`${path}.bounds must have positive width and height`)
    }
    if (bounds.x + bounds.width > 1 || bounds.y + bounds.height > 1) {
      issues.push(`${path}.bounds must remain inside the normalized pet frame`)
    }

    return [{ id, label, bounds, animation }]
  })
}

export const parsePetManifest = (input: unknown): PetManifest => {
  const issues: string[] = []
  const source = isRecord(input) ? input : {}
  if (!isRecord(input)) issues.push('manifest must be an object')

  if (source.formatVersion !== PET_MANIFEST_FORMAT_VERSION) {
    issues.push(`formatVersion must equal ${PET_MANIFEST_FORMAT_VERSION}`)
  }
  const id = readString(source.id, 'id', issues, { identifier: true }) ?? ''
  const displayName = readString(source.displayName, 'displayName', issues) ?? ''
  const description = readString(source.description, 'description', issues, { optional: true })
  const atlas = readAtlas(source.atlas, issues)
  const animations = readAnimations(source.animations, atlas, issues)
  const eventBindings = readEventBindings(source.eventBindings, animations, issues)
  const interactions = readInteractions(source.interactions, animations, issues)

  if (issues.length) throw new PetManifestValidationError(issues)
  return {
    formatVersion: PET_MANIFEST_FORMAT_VERSION,
    id,
    displayName,
    ...(description ? { description } : {}),
    atlas,
    animations,
    eventBindings,
    interactions,
  }
}
