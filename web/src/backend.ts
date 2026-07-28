import { invoke } from '@tauri-apps/api/core'

export const BACKEND_OPERATION_TIMEOUT_MS = 15_000

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
