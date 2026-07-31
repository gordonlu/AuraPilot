import {
  AnimatedSprite,
  Assets,
  Rectangle,
  Texture,
  type Application,
} from 'pixi.js'
import { createHatchPetManifest } from './hatchPet'
import type { PetAnimation, PetEventId, PetManifest } from './manifest'
import type { WorldSkinViewport } from '../runtime'

export const SEASCAPE_PET_CONFIG = Object.freeze({
  packagePath: 'pets/aura-starshell/',
  manifestFile: 'pet.json',
  rightInsetPx: 34,
  bottomInsetPx: 20,
  patrolLeftRatio: 0.62,
  patrolSpeedPxPerSecond: 34,
  restDurationMs: 4_800,
  referenceViewportWidth: 1200,
  baseScale: 0.72,
  minScale: 0.52,
  maxScale: 0.82,
})

type PetPatrolMode = 'resting' | 'walking-left' | 'walking-right'

export interface PetPlacement {
  left: number
  bottom: number
  width: number
  height: number
}

const boundedScale = (viewportWidth: number): number => Math.min(
  SEASCAPE_PET_CONFIG.maxScale,
  Math.max(
    SEASCAPE_PET_CONFIG.minScale,
    (viewportWidth / SEASCAPE_PET_CONFIG.referenceViewportWidth)
      * SEASCAPE_PET_CONFIG.baseScale,
  ),
)

const animationFrames = (
  texture: Texture,
  manifest: PetManifest,
  animation: PetAnimation,
) => animation.frames.map((column, index) => ({
  texture: new Texture({
    source: texture.source,
    frame: new Rectangle(
      column * manifest.atlas.cellWidth,
      animation.row * manifest.atlas.cellHeight,
      manifest.atlas.cellWidth,
      manifest.atlas.cellHeight,
    ),
  }),
  time: animation.durationsMs[index],
}))

const loadManifest = async (
  packageUrl: URL,
  signal: AbortSignal,
): Promise<PetManifest> => {
  const manifestUrl = new URL(SEASCAPE_PET_CONFIG.manifestFile, packageUrl)
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

  async mount(
    application: Application,
    packageUrl: URL,
    signal: AbortSignal,
  ): Promise<void> {
    const manifest = await loadManifest(packageUrl, signal)
    const atlasUrl = new URL(manifest.atlas.src, packageUrl).toString()
    const atlas = await Assets.load<Texture>(atlasUrl)
    if (signal.aborted) throw new DOMException('宠物加载已取消', 'AbortError')

    for (const [name, animation] of Object.entries(manifest.animations)) {
      const frames = animationFrames(atlas, manifest, animation)
      this.animations.set(name, frames)
      this.frameTextures.push(...frames.map((frame) => frame.texture))
    }

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
    this.viewport = viewport
    const scale = boundedScale(viewport.width)
    const maximumX = viewport.width - SEASCAPE_PET_CONFIG.rightInsetPx
    const minimumX = this.minimumPatrolX(viewport, scale)
    this.positionX = this.positionX === null
      ? maximumX
      : Math.min(maximumX, Math.max(minimumX, this.positionX))
    this.sprite.scale.set(scale)
    this.sprite.position.set(
      this.positionX,
      viewport.height - SEASCAPE_PET_CONFIG.bottomInsetPx,
    )
  }

  update(deltaMs: number): void {
    if (!this.sprite || !this.viewport || this.positionX === null) return
    if (this.patrolMode === 'resting') {
      this.restElapsedMs += deltaMs
      if (this.restElapsedMs < SEASCAPE_PET_CONFIG.restDurationMs) return
      this.restElapsedMs = 0
      const maximumX = this.viewport.width - SEASCAPE_PET_CONFIG.rightInsetPx
      this.patrolMode = this.positionX >= maximumX - 1
        ? 'walking-left'
        : 'walking-right'
      this.play(this.patrolMode === 'walking-left' ? 'running-left' : 'running-right')
      return
    }

    const direction = this.patrolMode === 'walking-left' ? -1 : 1
    const distance = SEASCAPE_PET_CONFIG.patrolSpeedPxPerSecond * (deltaMs / 1_000)
    const scale = this.sprite.scale.x
    const minimumX = this.minimumPatrolX(this.viewport, scale)
    const maximumX = this.viewport.width - SEASCAPE_PET_CONFIG.rightInsetPx
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
      bottom: SEASCAPE_PET_CONFIG.bottomInsetPx,
      width,
      height,
    }
  }

  play(animationName: string): void {
    if (!this.sprite || !this.manifest) return
    const animation = this.manifest.animations[animationName]
    const frames = this.animations.get(animationName)
    if (!animation || !frames) return

    this.sprite.textures = frames
    this.sprite.loop = animation.loop
    this.sprite.onComplete = animation.loop
      ? undefined
      : () => this.play('idle')
    this.sprite.gotoAndPlay(0)
  }

  interact(): void {
    const animation = this.manifest?.interactions[0]?.animation
    if (!animation) return
    this.patrolMode = 'resting'
    this.restElapsedMs = 0
    this.play(animation)
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
    for (const texture of this.frameTextures) texture.destroy(false)
    this.frameTextures = []
  }

  private minimumPatrolX(viewport: WorldSkinViewport, scale: number): number {
    const petWidth = (this.manifest?.atlas.cellWidth ?? 0) * scale
    return Math.max(
      petWidth + SEASCAPE_PET_CONFIG.rightInsetPx,
      viewport.width * SEASCAPE_PET_CONFIG.patrolLeftRatio,
    )
  }
}
