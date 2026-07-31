import {
  AnimatedSprite,
  Assets,
  Rectangle,
  Texture,
  type Application,
} from 'pixi.js'
import { createHatchPetManifest } from './hatchPet'
import type { PetAnimation, PetManifest } from './manifest'
import type { WorldSkinViewport } from '../runtime'

export const SEASCAPE_PET_CONFIG = Object.freeze({
  packagePath: 'pets/aura-starshell/',
  manifestFile: 'pet.json',
  rightInsetPx: 34,
  bottomInsetPx: 20,
  referenceViewportWidth: 1200,
  baseScale: 0.72,
  minScale: 0.52,
  maxScale: 0.82,
})

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
  }

  resize(viewport: WorldSkinViewport): void {
    if (!this.sprite) return
    const scale = boundedScale(viewport.width)
    this.sprite.scale.set(scale)
    this.sprite.position.set(
      viewport.width - SEASCAPE_PET_CONFIG.rightInsetPx,
      viewport.height - SEASCAPE_PET_CONFIG.bottomInsetPx,
    )
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
    this.animations.clear()
    for (const texture of this.frameTextures) texture.destroy(false)
    this.frameTextures = []
  }
}
