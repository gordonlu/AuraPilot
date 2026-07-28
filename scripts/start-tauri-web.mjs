import { spawn } from 'node:child_process'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

const configPath = new URL('../src-tauri/tauri.conf.json', import.meta.url)
const tauriConfig = JSON.parse(await readFile(configPath, 'utf8'))
const DEV_URL = tauriConfig.build.devUrl
const DEV_PORT = new URL(DEV_URL).port
const controller = new AbortController()
const timeout = setTimeout(() => controller.abort(), 1200)

let response
try {
  response = await fetch(DEV_URL, { signal: controller.signal })
} catch (error) {
  const code = error?.cause?.code
  if (code !== 'ECONNREFUSED') {
    console.error(`Cannot inspect ${DEV_URL}. Port ${DEV_PORT} may be occupied by another process.`)
    console.error('Stop that process, or verify it is the AuraPilot Vite server, then retry `pnpm tauri dev`.')
    process.exit(1)
  }
} finally {
  clearTimeout(timeout)
}

if (response) {
  const html = await response.text()
  if (response.ok && /<title>\s*AuraPilot\s*<\/title>/i.test(html)) {
    console.log(`Reusing the running AuraPilot dev server at ${DEV_URL}.`)
    process.exit(0)
  }
  console.error(`Port ${DEV_PORT} is already used by a service that is not AuraPilot.`)
  console.error('Stop the process using that port, then retry `pnpm tauri dev`.')
  process.exit(1)
}

const webDirectory = fileURLToPath(new URL('../web/', import.meta.url))
const viteExecutable = fileURLToPath(new URL('../web/node_modules/vite/bin/vite.js', import.meta.url))
const child = spawn(process.execPath, [viteExecutable], {
  cwd: webDirectory,
  stdio: 'inherit',
})

let stopping = false
for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => {
    stopping = true
    if (!child.killed) child.kill(signal)
  })
}
child.on('error', (error) => {
  console.error(`Unable to start the AuraPilot Vite server: ${error.message}`)
  process.exit(1)
})
child.on('exit', (code, signal) => {
  if (stopping || signal === 'SIGINT' || signal === 'SIGTERM') process.exit(0)
  process.exit(code ?? 1)
})
