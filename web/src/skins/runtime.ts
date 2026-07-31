import { WORLD_SKIN_CONFIG, type WorldSkin } from './worldSkin'

export type WorldSkinRuntimeStatus = 'idle' | 'loading' | 'ready' | 'error'
export type WorldSkinPauseReason = 'document-hidden' | 'host-unmounted' | 'skin-changed'

export interface WorldSkinViewport {
  width: number
  height: number
  devicePixelRatio: number
}

export interface WorldSkinRuntimeContext {
  signal: AbortSignal
}

export interface WorldSkinRuntime {
  mount(container: HTMLElement, context: WorldSkinRuntimeContext): Promise<void>
  resize(viewport: WorldSkinViewport): void
  pause(reason: WorldSkinPauseReason): void
  resume(): void
  dispose(): void
}

export interface WorldSkinRuntimeModule {
  createWorldSkinRuntime(): WorldSkinRuntime
}

export type WorldSkinRuntimeLoader = (
  skin: Exclude<WorldSkin, 'classic'>,
) => Promise<WorldSkinRuntimeModule>

export interface WorldSkinRuntimeState {
  skin: WorldSkin
  status: WorldSkinRuntimeStatus
  error: string | null
}

export interface WorldSkinControllerOptions {
  loadRuntime: WorldSkinRuntimeLoader
  timeoutMs?: number
  onStateChange?: (state: WorldSkinRuntimeState) => void
}

const errorMessage = (error: unknown): string =>
  error instanceof Error && error.message ? error.message : String(error)

export class WorldSkinController {
  private runtime: WorldSkinRuntime | null = null
  private abortController: AbortController | null = null
  private activation = 0
  private readonly timeoutMs: number
  private readonly loadRuntime: WorldSkinRuntimeLoader
  private readonly onStateChange?: (state: WorldSkinRuntimeState) => void

  constructor(options: WorldSkinControllerOptions) {
    this.loadRuntime = options.loadRuntime
    this.timeoutMs = options.timeoutMs ?? WORLD_SKIN_CONFIG.loadTimeoutMs
    this.onStateChange = options.onStateChange
  }

  async activate(skin: WorldSkin, container: HTMLElement): Promise<void> {
    const activation = ++this.activation
    this.disposeRuntime('skin-changed')

    if (skin === 'classic') {
      this.publish({ skin, status: 'idle', error: null })
      return
    }

    const abortController = new AbortController()
    this.abortController = abortController
    this.publish({ skin, status: 'loading', error: null })
    let candidate: WorldSkinRuntime | null = null
    let candidateDisposed = false
    let timeout: ReturnType<typeof setTimeout> | null = null
    const disposeCandidate = () => {
      if (!candidate || candidateDisposed) return
      candidateDisposed = true
      candidate.dispose()
    }

    try {
      await Promise.race([
        (async () => {
          const module = await this.loadRuntime(skin)
          if (activation !== this.activation || abortController.signal.aborted) return
          candidate = module.createWorldSkinRuntime()
          await candidate.mount(container, { signal: abortController.signal })
        })(),
        new Promise<never>((_, reject) => {
          timeout = setTimeout(
            () => {
              abortController.abort()
              reject(new Error(`加载世界皮肤超过 ${this.timeoutMs}ms`))
            },
            this.timeoutMs,
          )
        }),
      ])
      if (activation !== this.activation || abortController.signal.aborted) {
        disposeCandidate()
        return
      }

      this.runtime = candidate
      candidate = null
      this.publish({ skin, status: 'ready', error: null })
    } catch (error) {
      abortController.abort()
      disposeCandidate()
      if (activation !== this.activation) return
      this.abortController = null
      this.publish({
        skin,
        status: 'error',
        error: `无法启动${skin === 'seascape' ? '海岸世界' : '世界皮肤'}：${errorMessage(error)}`,
      })
    } finally {
      if (timeout) clearTimeout(timeout)
    }
  }

  resize(viewport: WorldSkinViewport): void {
    this.runtime?.resize(viewport)
  }

  pause(reason: WorldSkinPauseReason): void {
    this.runtime?.pause(reason)
  }

  resume(): void {
    this.runtime?.resume()
  }

  dispose(reason: WorldSkinPauseReason = 'host-unmounted'): void {
    ++this.activation
    this.disposeRuntime(reason)
  }

  private disposeRuntime(reason: WorldSkinPauseReason): void {
    this.abortController?.abort()
    this.abortController = null
    if (!this.runtime) return
    this.runtime.pause(reason)
    this.runtime.dispose()
    this.runtime = null
  }

  private publish(state: WorldSkinRuntimeState): void {
    this.onStateChange?.(state)
  }
}
