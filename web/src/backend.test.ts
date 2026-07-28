import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  BACKEND_OPERATION_TIMEOUT_MS,
  BackendTimeoutError,
  withBackendTimeout,
} from './backend'

describe('backend operation timeout', () => {
  afterEach(() => vi.useRealTimers())

  it('keeps the timeout centralized and rejects a stalled operation visibly', async () => {
    vi.useFakeTimers()
    const result = withBackendTimeout(new Promise<never>(() => undefined), 'scan_project')
    const assertion = expect(result).rejects.toEqual(expect.any(BackendTimeoutError))

    await vi.advanceTimersByTimeAsync(BACKEND_OPERATION_TIMEOUT_MS)

    await assertion
  })

  it('returns completed backend work and clears its timeout', async () => {
    vi.useFakeTimers()
    await expect(withBackendTimeout(Promise.resolve('done'), 'create_task')).resolves.toBe('done')
    expect(vi.getTimerCount()).toBe(0)
  })
})
