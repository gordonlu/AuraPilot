import { isTauri } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { defineStore } from 'pinia'
import { invokeBackend, withBackendTimeout } from '../backend'
import type { PendingItem, PendingTarget } from '../types/protocol'
import { APPROVAL_EVENT } from './agents'
import { PROJECT_CHANGED_EVENT } from './projects'

let refreshTimer: ReturnType<typeof setTimeout> | null = null

export const usePendingStore = defineStore('pending', {
  state: () => ({
    items: [] as PendingItem[],
    loading: false,
    refreshQueued: false,
    error: null as string | null,
    stopProjectListening: null as UnlistenFn | null,
    stopApprovalListening: null as UnlistenFn | null,
    lastUpdated: null as Date | null,
  }),
  getters: {
    count: (state) => state.items.length,
    byProject: (state) => {
      const groups = new Map<string, { name: string; items: PendingItem[] }>()
      for (const item of state.items) {
        if (!groups.has(item.project_id)) groups.set(item.project_id, { name: item.project_name, items: [] })
        groups.get(item.project_id)!.items.push(item)
      }
      return [...groups.entries()].map(([id, group]) => ({ id, ...group }))
    },
  },
  actions: {
    targetFor(item: PendingItem): PendingTarget {
      return item.kind === 'approval'
        ? { view: 'execution', project_id: item.project_id, approval_id: item.approval_id, path: null }
        : { view: 'diagnostics', project_id: item.project_id, approval_id: null, path: item.path }
    },
    async refresh() {
      if (!isTauri()) return
      if (this.loading) {
        this.refreshQueued = true
        return
      }
      this.loading = true
      this.error = null
      try {
        this.items = await invokeBackend<PendingItem[]>('list_pending_items')
        this.lastUpdated = new Date()
      } catch (error) {
        this.error = `待处理事项刷新失败：${String(error)}`
      } finally {
        this.loading = false
        if (this.refreshQueued) {
          this.refreshQueued = false
          void this.refresh()
        }
      }
    },
    scheduleRefresh() {
      if (refreshTimer) clearTimeout(refreshTimer)
      refreshTimer = setTimeout(() => {
        refreshTimer = null
        void this.refresh()
      }, 150)
    },
    async startWatching() {
      if (!isTauri() || this.stopProjectListening) return
      await this.refresh()
      try {
        this.stopProjectListening = await withBackendTimeout(
          listen(PROJECT_CHANGED_EVENT, () => this.scheduleRefresh()),
          'listen_pending_project_changes',
        )
        this.stopApprovalListening = await withBackendTimeout(
          listen(APPROVAL_EVENT, () => this.scheduleRefresh()),
          'listen_pending_approvals',
        )
      } catch (error) {
        this.error = `无法监听待处理事项：${String(error)}`
      }
    },
    stopWatching() {
      this.stopProjectListening?.()
      this.stopApprovalListening?.()
      this.stopProjectListening = null
      this.stopApprovalListening = null
      if (refreshTimer) clearTimeout(refreshTimer)
      refreshTimer = null
    },
  },
})
