import { spawn } from 'node:child_process'

const DEV_URL = 'http://localhost:1420'
const controller = new AbortController()
const timeout = setTimeout(() => controller.abort(), 1200)

let response
try {
  response = await fetch(DEV_URL, { signal: controller.signal })
} catch (error) {
  const code = error?.cause?.code
  if (code !== 'ECONNREFUSED') {
    console.error(`Cannot inspect ${DEV_URL}. Port 1420 may be occupied by another process.`)
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
  console.error(`Port 1420 is already used by a service that is not AuraPilot.`)
  console.error('Stop the process using that port, then retry `pnpm tauri dev`.')
  process.exit(1)
}

const executable = process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm'
const child = spawn(executable, ['--dir', 'web', 'dev'], { stdio: 'inherit' })

for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => child.kill(signal))
}
child.on('error', (error) => {
  console.error(`Unable to start the AuraPilot Vite server: ${error.message}`)
  process.exit(1)
})
child.on('exit', (code, signal) => {
  if (signal) process.kill(process.pid, signal)
  else process.exit(code ?? 1)
})
