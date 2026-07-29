import { invoke } from '@tauri-apps/api/core'

// CoreConfig caps provider startup at 15 seconds. This leaves enough time for
// the backend to persist and return the actionable failure instead of racing it.
export const BACKEND_OPERATION_TIMEOUT_MS = 20_000
export const LONG_BACKEND_OPERATION_TIMEOUT_MS = 120_000

export class BackendTimeoutError extends Error {
  constructor(command: string) {
    super(`操作超时（${command}），界面已恢复响应。操作可能仍在后台完成，请确认当前状态后再重试。`)
    this.name = 'BackendTimeoutError'
  }
}

export const withBackendTimeout = async <T>(operation: Promise<T>, command: string): Promise<T> => {
  let timeout: ReturnType<typeof setTimeout> | undefined
  try {
    return await Promise.race([
      operation,
      new Promise<never>((_, reject) => {
        timeout = setTimeout(
          () => reject(new BackendTimeoutError(command)),
          BACKEND_OPERATION_TIMEOUT_MS,
        )
      }),
    ])
  } finally {
    if (timeout) clearTimeout(timeout)
  }
}

export const invokeBackend = <T>(command: string, args?: Record<string, unknown>) =>
  withBackendTimeout(invoke<T>(command, args), command)

export const invokeLongBackend = <T>(command: string, args?: Record<string, unknown>) => {
  let timeout: ReturnType<typeof setTimeout> | undefined
  return Promise.race([
    invoke<T>(command, args),
    new Promise<never>((_, reject) => {
      timeout = setTimeout(
        () => reject(new BackendTimeoutError(command)),
        LONG_BACKEND_OPERATION_TIMEOUT_MS,
      )
    }),
  ]).finally(() => {
    if (timeout) clearTimeout(timeout)
  })
}
