import { describe, expect, it, vi } from 'vitest'
import {
  WorldSkinController,
  type WorldSkinRuntime,
  type WorldSkinRuntimeState,
} from './runtime'

const runtime = (): WorldSkinRuntime => ({
  mount: vi.fn().mockResolvedValue(undefined),
  resize: vi.fn(),
  pause: vi.fn(),
  resume: vi.fn(),
  dispose: vi.fn(),
})

describe('world skin runtime controller', () => {
  it('keeps classic mode free of runtime loading', async () => {
    const loadRuntime = vi.fn()
    const states: WorldSkinRuntimeState[] = []
    const controller = new WorldSkinController({
      loadRuntime,
      onStateChange: (state) => states.push(state),
    })

    await controller.activate('classic', document.createElement('div'))

    expect(loadRuntime).not.toHaveBeenCalled()
    expect(states.at(-1)).toEqual({ skin: 'classic', status: 'idle', error: null })
  })

  it('loads and mounts the selected runtime before reporting ready', async () => {
    const instance = runtime()
    const states: WorldSkinRuntimeState[] = []
    const controller = new WorldSkinController({
      loadRuntime: vi.fn().mockResolvedValue({ createWorldSkinRuntime: () => instance }),
      onStateChange: (state) => states.push(state),
    })
    const host = document.createElement('div')

    await controller.activate('seascape', host)

    expect(instance.mount).toHaveBeenCalledOnce()
    expect(states.map((state) => state.status)).toEqual(['loading', 'ready'])
    controller.pause('document-hidden')
    controller.resume()
    controller.dispose()
    expect(instance.pause).toHaveBeenCalledTimes(2)
    expect(instance.resume).toHaveBeenCalledOnce()
    expect(instance.dispose).toHaveBeenCalledOnce()
  })

  it('makes startup failures observable and disposes partial runtimes', async () => {
    const instance = runtime()
    vi.mocked(instance.mount).mockRejectedValue(new Error('WebGL unavailable'))
    const states: WorldSkinRuntimeState[] = []
    const controller = new WorldSkinController({
      loadRuntime: vi.fn().mockResolvedValue({ createWorldSkinRuntime: () => instance }),
      onStateChange: (state) => states.push(state),
    })

    await controller.activate('seascape', document.createElement('div'))

    expect(instance.dispose).toHaveBeenCalledOnce()
    expect(states.at(-1)).toEqual({
      skin: 'seascape',
      status: 'error',
      error: '无法启动海岸世界：WebGL unavailable',
    })
  })

  it('times out a runtime that never finishes mounting', async () => {
    const instance = runtime()
    vi.mocked(instance.mount).mockImplementation(() => new Promise<void>(() => undefined))
    const states: WorldSkinRuntimeState[] = []
    const controller = new WorldSkinController({
      loadRuntime: vi.fn().mockResolvedValue({ createWorldSkinRuntime: () => instance }),
      timeoutMs: 5,
      onStateChange: (state) => states.push(state),
    })

    await controller.activate('seascape', document.createElement('div'))

    expect(instance.dispose).toHaveBeenCalledOnce()
    expect(states.at(-1)?.status).toBe('error')
    expect(states.at(-1)?.error).toContain('加载世界皮肤超过 5ms')
  })

  it('aborts a stale mount when the user returns to classic mode', async () => {
    let finishMount: (() => void) | undefined
    const instance = runtime()
    vi.mocked(instance.mount).mockImplementation(() => new Promise<void>((resolve) => {
      finishMount = resolve
    }))
    const controller = new WorldSkinController({
      loadRuntime: vi.fn().mockResolvedValue({ createWorldSkinRuntime: () => instance }),
    })

    const activation = controller.activate('seascape', document.createElement('div'))
    await Promise.resolve()
    await Promise.resolve()
    await controller.activate('classic', document.createElement('div'))
    finishMount?.()
    await activation

    expect(instance.dispose).toHaveBeenCalledOnce()
  })
})
