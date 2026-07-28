import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'
import tauriConfig from '../src-tauri/tauri.conf.json'

const devUrl = new URL(tauriConfig.build.devUrl)

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    host: devUrl.hostname,
    port: Number(devUrl.port),
    strictPort: true,
  },
  test: {
    environment: 'jsdom',
  },
})
