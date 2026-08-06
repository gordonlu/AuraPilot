import {
  AnimatedSprite,
  CanvasSource,
  Texture,
  type Application,
} from 'pixi.js'
import { createHatchPetManifest } from './hatchPet'
import type { PetAnimation, PetEventId, PetManifest } from './manifest'
import type { WorldSkinViewport } from '../runtime'

export interface PetActorConfig {
  packagePath: string
  manifestFile: string
  rightInsetPx: number
  bottomInsetPx: number
  patrolLeftRatio: number
  patrolRightRatio?: number
  initialEdge?: 'left' | 'right'
  patrolSpeedPxPerSecond: number
  restDurationMs: number
  interactionAnimation?: string
  referenceViewportWidth: number
  baseScale: number
  minScale: number
  maxScale: number
}

export const SEASCAPE_PET_CONFIG: Readonly<PetActorConfig> = Object.freeze({
  packagePath: 'pets/aura-starshell/',
  manifestFile: 'pet.json',
  rightInsetPx: 34,
  bottomInsetPx: 20,
  patrolLeftRatio: 0.62,
  patrolSpeedPxPerSecond: 34,
  restDurationMs: 4_800,
  referenceViewportWidth: 1200,
  baseScale: 0.72,
  minScale: 0.38,
  maxScale: 0.82,
})

export const STELLAR_PET_CONFIG: Readonly<PetActorConfig> = Object.freeze({
  packagePath: 'pets/aura-navigator/',
  manifestFile: 'pet.json',
  rightInsetPx: 34,
  bottomInsetPx: 20,
  patrolLeftRatio: 0.62,
  patrolSpeedPxPerSecond: 30,
  restDurationMs: 5_200,
  referenceViewportWidth: 1200,
  baseScale: 0.72,
  minScale: 0.38,
  maxScale: 0.82,
})

export const POLARSCAPE_PET_CONFIG: Readonly<PetActorConfig> = Object.freeze({
  packagePath: 'pets/aura-snowfox/',
  manifestFile: 'pet.json',
  rightInsetPx: 28,
  bottomInsetPx: 18,
  patrolLeftRatio: 0.06,
  patrolRightRatio: 0.42,
  initialEdge: 'left',
  patrolSpeedPxPerSecond: 28,
  restDurationMs: 5_600,
  interactionAnimation: 'waiting',
  referenceViewportWidth: 1200,
  baseScale: 0.68,
  minScale: 0.36,
  maxScale: 0.78,
})

type PetPatrolMode = 'resting' | 'walking-left' | 'walking-right'

export interface PetPlacement {
  left: number
  bottom: number
  width: number
  height: number
}

const boundedScale = (viewportWidth: number, config: Readonly<PetActorConfig>): number => Math.min(
  config.maxScale,
  Math.max(
    config.minScale,
    (viewportWidth / config.referenceViewportWidth)
      * config.baseScale,
  ),
)

export const remapPatrolPosition = (
  position: number,
  previousMinimum: number,
  previousMaximum: number,
  nextMinimum: number,
  nextMaximum: number,
): number => {
  const previousRange = previousMaximum - previousMinimum
  const progress = previousRange <= 0
    ? 1
    : Math.min(1, Math.max(0, (position - previousMinimum) / previousRange))
  return nextMinimum + progress * (nextMaximum - nextMinimum)
}

const frameKey = (row: number, column: number): string => `${row}:${column}`

const animationFrames = (
  textures: ReadonlyMap<string, Texture>,
  manifest: PetManifest,
  animation: PetAnimation,
) => animation.frames.map((column, index) => ({
  texture: textures.get(frameKey(animation.row, column))
    ?? (() => { throw new Error(`宠物动画帧缺失：${animation.row}:${column}`) })(),
  time: animation.durationsMs[index],
}))

const loadAtlasImage = (url: string, signal: AbortSignal): Promise<HTMLImageElement> =>
  new Promise((resolve, reject) => {
    const image = new Image()
    image.decoding = 'async'

    const cleanup = () => {
      signal.removeEventListener('abort', abort)
      image.onload = null
      image.onerror = null
    }
    const abort = () => {
      cleanup()
      image.src = ''
      reject(new DOMException('宠物加载已取消', 'AbortError'))
    }

    if (signal.aborted) {
      abort()
      return
    }
    signal.addEventListener('abort', abort, { once: true })
    image.onload = () => {
      cleanup()
      resolve(image)
    }
    image.onerror = () => {
      cleanup()
      reject(new Error('宠物图集解码失败，无法显示角色'))
    }
    image.src = url
  })

const sliceAtlasFrames = async (
  atlasUrl: string,
  manifest: PetManifest,
  signal: AbortSignal,
): Promise<Map<string, Texture>> => {
  const image = await loadAtlasImage(atlasUrl, signal)
  if (image.naturalWidth !== manifest.atlas.width || image.naturalHeight !== manifest.atlas.height) {
    throw new Error(
      `宠物图集尺寸不匹配：期望 ${manifest.atlas.width}×${manifest.atlas.height}，`
      + `实际 ${image.naturalWidth}×${image.naturalHeight}`,
    )
  }

  const cells = new Set<string>()
  for (const animation of Object.values(manifest.animations)) {
    for (const column of animation.frames) cells.add(frameKey(animation.row, column))
  }

  const textures = new Map<string, Texture>()
  try {
    for (const key of cells) {
      if (signal.aborted) throw new DOMException('宠物加载已取消', 'AbortError')
      const [row, column] = key.split(':').map(Number)
      const canvas = document.createElement('canvas')
      canvas.width = manifest.atlas.cellWidth
      canvas.height = manifest.atlas.cellHeight
      const context = canvas.getContext('2d', { alpha: true })
      if (!context) throw new Error('浏览器无法创建宠物 Canvas 纹理')
      context.clearRect(0, 0, canvas.width, canvas.height)
      context.drawImage(
        image,
        column * manifest.atlas.cellWidth,
        row * manifest.atlas.cellHeight,
        manifest.atlas.cellWidth,
        manifest.atlas.cellHeight,
        0,
        0,
        manifest.atlas.cellWidth,
        manifest.atlas.cellHeight,
      )
      textures.set(key, new Texture({
        source: new CanvasSource({
          resource: canvas,
          transparent: true,
          autoGarbageCollect: false,
          label: `${manifest.id}-${key}`,
        }),
      }))
    }
    return textures
  } catch (error) {
    for (const texture of textures.values()) texture.destroy(true)
    throw error
  }
}

const loadManifest = async (
  packageUrl: URL,
  manifestFile: string,
  signal: AbortSignal,
): Promise<PetManifest> => {
  const manifestUrl = new URL(manifestFile, packageUrl)
  const response = await fetch(manifestUrl, { signal })
  if (!response.ok) {
    throw new Error(`宠物清单加载失败：${response.status} ${response.statusText}`)
  }
  return createHatchPetManifest(await response.json())
}

export class PetActor {
  private sprite: AnimatedSprite | null = null
  private manifest: PetManifest | null = null
  private animations = new Map<string, ReturnType<typeof animationFrames>>()
  private frameTextures: Texture[] = []
  private viewport: WorldSkinViewport | null = null
  private patrolMode: PetPatrolMode = 'resting'
  private restElapsedMs = 0
  private positionX: number | null = null

  constructor(private readonly config: Readonly<PetActorConfig> = SEASCAPE_PET_CONFIG) {}

  async mount(
    application: Application,
    packageUrl: URL,
    signal: AbortSignal,
  ): Promise<void> {
    const manifest = await loadManifest(packageUrl, this.config.manifestFile, signal)
    const atlasUrl = new URL(manifest.atlas.src, packageUrl).toString()
    const textures = await sliceAtlasFrames(atlasUrl, manifest, signal)
    if (signal.aborted) throw new DOMException('宠物加载已取消', 'AbortError')

    for (const [name, animation] of Object.entries(manifest.animations)) {
      const frames = animationFrames(textures, manifest, animation)
      this.animations.set(name, frames)
    }
    this.frameTextures.push(...textures.values())

    const idleFrames = this.animations.get('idle')
    if (!idleFrames) throw new Error('宠物缺少 idle 动画')

    const sprite = new AnimatedSprite({
      textures: idleFrames,
      autoPlay: true,
      loop: true,
    })
    sprite.anchor.set(1, 1)
    sprite.label = manifest.displayName
    application.stage.addChild(sprite)

    this.manifest = manifest
    this.sprite = sprite
    this.play('idle')
  }

  resize(viewport: WorldSkinViewport): void {
    if (!this.sprite) return
    const previousViewport = this.viewport
    const previousScale = this.sprite.scale.x
    const previousPosition = this.positionX
    const scale = boundedScale(viewport.width, this.config)
    const maximumX = this.maximumPatrolX(viewport)
    const minimumX = this.minimumPatrolX(viewport, scale)
    this.positionX = previousPosition === null || previousViewport === null
      ? this.config.initialEdge === 'left' ? minimumX : maximumX
      : remapPatrolPosition(
          previousPosition,
          this.minimumPatrolX(previousViewport, previousScale),
          this.maximumPatrolX(previousViewport),
          minimumX,
          maximumX,
        )
    this.viewport = viewport
    this.sprite.scale.set(scale)
    this.sprite.position.set(
      this.positionX,
      viewport.height - this.config.bottomInsetPx,
    )
  }

  update(deltaMs: number): void {
    if (!this.sprite || !this.viewport || this.positionX === null) return
    if (this.patrolMode === 'resting') {
      this.restElapsedMs += deltaMs
      if (this.restElapsedMs < this.config.restDurationMs) return
      this.restElapsedMs = 0
      const maximumX = this.maximumPatrolX(this.viewport)
      this.patrolMode = this.positionX >= maximumX - 1
        ? 'walking-left'
        : 'walking-right'
      this.play(this.patrolMode === 'walking-left' ? 'running-left' : 'running-right')
      return
    }

    const direction = this.patrolMode === 'walking-left' ? -1 : 1
    const distance = this.config.patrolSpeedPxPerSecond * (deltaMs / 1_000)
    const scale = this.sprite.scale.x
    const minimumX = this.minimumPatrolX(this.viewport, scale)
    const maximumX = this.maximumPatrolX(this.viewport)
    this.positionX = Math.min(maximumX, Math.max(minimumX, this.positionX + direction * distance))
    this.sprite.x = this.positionX

    const reachedBoundary = direction < 0
      ? this.positionX <= minimumX
      : this.positionX >= maximumX
    if (reachedBoundary) {
      this.patrolMode = 'resting'
      this.restElapsedMs = 0
      this.play('idle')
    }
  }

  placement(): PetPlacement | null {
    if (!this.sprite || !this.viewport || this.positionX === null) return null
    const width = this.manifest!.atlas.cellWidth * this.sprite.scale.x
    const height = this.manifest!.atlas.cellHeight * this.sprite.scale.y
    return {
      left: this.positionX - width,
      bottom: this.config.bottomInsetPx,
      width,
      height,
    }
  }

  play(animationName: string, loopOverride?: boolean): void {
    if (!this.sprite || !this.manifest) return
    const animation = this.manifest.animations[animationName]
    const frames = this.animations.get(animationName)
    if (!animation || !frames) return

    this.sprite.textures = frames
    const loop = loopOverride ?? animation.loop
    this.sprite.loop = loop
    this.sprite.onComplete = loop
      ? undefined
      : () => this.play('idle')
    this.sprite.gotoAndPlay(0)
  }

  interact(): void {
    const animation = this.config.interactionAnimation
      ?? this.manifest?.interactions[0]?.animation
    if (!animation) return
    this.patrolMode = 'resting'
    this.restElapsedMs = 0
    this.play(animation, false)
  }

  playEvent(event: PetEventId): void {
    const animation = this.manifest?.eventBindings[event]
    if (!animation) return
    this.patrolMode = 'resting'
    this.restElapsedMs = 0
    this.play(animation)
  }

  pause(): void {
    this.sprite?.stop()
  }

  resume(): void {
    this.sprite?.play()
  }

  dispose(): void {
    this.sprite?.removeFromParent()
    this.sprite?.destroy()
    this.sprite = null
    this.manifest = null
    this.viewport = null
    this.positionX = null
    this.patrolMode = 'resting'
    this.restElapsedMs = 0
    this.animations.clear()
    for (const texture of this.frameTextures) texture.destroy(true)
    this.frameTextures = []
  }

  private minimumPatrolX(viewport: WorldSkinViewport, scale: number): number {
    const petWidth = (this.manifest?.atlas.cellWidth ?? 0) * scale
    return Math.max(
      petWidth + this.config.rightInsetPx,
      viewport.width * this.config.patrolLeftRatio,
    )
  }

  private maximumPatrolX(viewport: WorldSkinViewport): number {
    return Math.max(
      this.minimumPatrolX(viewport, this.sprite?.scale.x ?? this.config.minScale),
      Math.min(
        viewport.width - this.config.rightInsetPx,
        viewport.width * (this.config.patrolRightRatio ?? 1),
      ),
    )
  }
}
