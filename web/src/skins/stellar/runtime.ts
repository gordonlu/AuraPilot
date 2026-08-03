import { Application } from 'pixi.js'
import type {
  WorldSkinPauseReason,
  WorldSkinRuntime,
  WorldSkinRuntimeContext,
  WorldSkinRuntimeEvent,
  WorldSkinViewport,
} from '../runtime'
import { PetActor, STELLAR_PET_CONFIG } from '../pets/actor'

class StellarRuntime implements WorldSkinRuntime {
  private application: Application | null = null
  private readonly pet = new PetActor(STELLAR_PET_CONFIG)
  private viewport: WorldSkinViewport | null = null
  private container: HTMLElement | null = null
  private readonly updatePet = (ticker: { deltaMS: number }) => {
    this.pet.update(ticker.deltaMS)
    this.syncPetPlacement()
  }

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
      const packageUrl = new URL(STELLAR_PET_CONFIG.packagePath, document.baseURI)
      await this.pet.mount(application, packageUrl, context.signal)
      if (context.signal.aborted) {
        this.pet.dispose()
        application.destroy({ removeView: true }, { children: true })
        return
      }

      this.container = container
      application.ticker.add(this.updatePet)
      this.application = application
      if (this.viewport) {
        this.pet.resize(this.viewport)
        this.syncPetPlacement()
      }
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
    this.syncPetPlacement()
  }

  pause(_reason: WorldSkinPauseReason): void {
    this.pet.pause()
    this.application?.stop()
  }

  resume(): void {
    this.application?.start()
    this.pet.resume()
  }

  dispatch(event: WorldSkinRuntimeEvent): void {
    if (event.type === 'pet-interact') this.pet.interact()
    if (event.type === 'pet-state') this.pet.playEvent(event.event)
  }

  dispose(): void {
    this.application?.ticker.remove(this.updatePet)
    this.pet.dispose()
    this.application?.destroy(
      { removeView: true },
      { children: true, texture: true, textureSource: true },
    )
    this.application = null
    this.viewport = null
    this.container?.style.removeProperty('--world-pet-left')
    this.container?.style.removeProperty('--world-pet-bottom')
    this.container?.style.removeProperty('--world-pet-width')
    this.container?.style.removeProperty('--world-pet-height')
    this.container = null
  }

  private syncPetPlacement(): void {
    const placement = this.pet.placement()
    if (!placement || !this.container) return
    this.container.style.setProperty('--world-pet-left', `${Math.round(placement.left)}px`)
    this.container.style.setProperty('--world-pet-bottom', `${Math.round(placement.bottom)}px`)
    this.container.style.setProperty('--world-pet-width', `${Math.round(placement.width)}px`)
    this.container.style.setProperty('--world-pet-height', `${Math.round(placement.height)}px`)
  }
}

export const createWorldSkinRuntime = (): WorldSkinRuntime => new StellarRuntime()
