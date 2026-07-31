import { Application } from 'pixi.js'
import type {
  WorldSkinPauseReason,
  WorldSkinRuntime,
  WorldSkinRuntimeContext,
  WorldSkinViewport,
} from '../runtime'
import { PetActor, SEASCAPE_PET_CONFIG } from '../pets/actor'

class SeascapeRuntime implements WorldSkinRuntime {
  private application: Application | null = null
  private readonly pet = new PetActor()
  private viewport: WorldSkinViewport | null = null

  async mount(container: HTMLElement, context: WorldSkinRuntimeContext): Promise<void> {
    const application = new Application()
    try {
      await application.init({
        resizeTo: container,
        preference: 'webgl',
        backgroundAlpha: 0,
        antialias: true,
        autoDensity: true,
        resolution: Math.min(window.devicePixelRatio || 1, 2),
      })
      if (context.signal.aborted) {
        application.destroy({ removeView: true }, { children: true })
        return
      }

      application.canvas.className = 'world-skin-canvas'
      application.canvas.setAttribute('aria-hidden', 'true')
      container.append(application.canvas)
      const packageUrl = new URL(SEASCAPE_PET_CONFIG.packagePath, document.baseURI)
      await this.pet.mount(application, packageUrl, context.signal)
      if (context.signal.aborted) {
        this.pet.dispose()
        application.destroy({ removeView: true }, { children: true })
        return
      }
      this.application = application
      if (this.viewport) this.pet.resize(this.viewport)
    } catch (error) {
      this.pet.dispose()
      application.destroy({ removeView: true }, { children: true })
      throw error
    }
  }

  resize(viewport: WorldSkinViewport): void {
    this.viewport = viewport
    this.application?.renderer.resize(viewport.width, viewport.height)
    this.pet.resize(viewport)
  }

  pause(_reason: WorldSkinPauseReason): void {
    this.pet.pause()
    this.application?.stop()
  }

  resume(): void {
    this.application?.start()
    this.pet.resume()
  }

  dispose(): void {
    this.pet.dispose()
    this.application?.destroy(
      { removeView: true },
      { children: true, texture: true, textureSource: true },
    )
    this.application = null
    this.viewport = null
  }
}

export const createWorldSkinRuntime = (): WorldSkinRuntime => new SeascapeRuntime()
