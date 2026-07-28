import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const executable = fileURLToPath(new URL('../node_modules/@tauri-apps/cli/tauri.js', import.meta.url))
const interactive = Boolean(process.stdin.isTTY && process.stdin.setRawMode)
const child = spawn(process.execPath, [executable, ...process.argv.slice(2)], {
  stdio: [interactive ? 'pipe' : 'inherit', 'inherit', 'inherit'],
})

let stopping = false
const stop = (signal = 'SIGINT') => {
  stopping = true
  if (!child.killed) child.kill(signal)
}
const restoreTerminal = () => {
  if (!interactive) return
  process.stdin.setRawMode(false)
  process.stdin.pause()
}

if (interactive) {
  process.stdin.setRawMode(true)
  process.stdin.resume()
  process.stdin.on('data', (data) => {
    if (data.includes(3)) stop()
    else child.stdin.write(data)
  })
}
for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => stop(signal))
}
child.on('error', (error) => {
  restoreTerminal()
  console.error(`Unable to start Tauri: ${error.message}`)
  process.exit(1)
})
child.on('exit', (code, signal) => {
  restoreTerminal()
  if (stopping || signal === 'SIGINT' || signal === 'SIGTERM') process.exit(0)
  process.exit(code ?? 1)
})
