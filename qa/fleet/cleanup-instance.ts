import { readFile, realpath, rm } from 'node:fs/promises'
import { join } from 'node:path'

const home = required('FLEET_HOME')
const id = required('FLEET_INSTANCE_ID')
const workspace = join(home, '.ducktape', 'workspaces', id)
const pidfile = join(workspace, 'node.pid')
const configPath = join(workspace, 'node.toml')

let pid: number
try { pid = Number((await readFile(pidfile, 'utf8')).trim()) } catch { process.exit(0) }
if (!Number.isSafeInteger(pid) || pid <= 1) throw new Error('workspace node pidfile is invalid')

const owned = await identity(pid)
if (!owned) { await rm(pidfile, { force: true }); process.exit(0) }
const expectedExecutable = await realpath(join(required('FLEET_ARTIFACT_DIR'), 'bin', 'ducktape-node'))
const config = await realpath(configPath)
if (owned.pgid !== pid) throw new Error('workspace node is not the leader of its process group')
const executable = await processExecutable(pid)
if (executable !== expectedExecutable || !await processUsesConfig(pid, [configPath, config])) {
  throw new Error('workspace node pidfile does not identify this Fleet instance')
}

try {
  const registry = JSON.parse(await readFile(join(home, '.ducktape', 'registry.json'), 'utf8')) as {
    workspaces?: Array<{ id?: string; ports?: { http?: number } }>
  }
  const port = registry.workspaces?.find((item) => item.id === id)?.ports?.http
  if (Number.isInteger(port) && port! > 0 && port! <= 65_535) {
    await fetch(`http://127.0.0.1:${port}/v1/admin/shutdown`, { method: 'POST', signal: AbortSignal.timeout(500) }).catch(() => undefined)
  }
} catch { /* exact process teardown below remains authoritative */ }

if (await sameProcess(pid, owned.startTime)) {
  signalGroup(owned.pgid, 'SIGTERM')
  await waitForExit(owned.pgid, 5_000)
}
if (groupAlive(owned.pgid)) {
  if (await sameProcess(pid, owned.startTime) || !await identity(pid)) signalGroup(owned.pgid, 'SIGKILL')
  else throw new Error('workspace node PID was reused before cleanup completed')
  await waitForExit(owned.pgid, 5_000)
}
if (groupAlive(owned.pgid)) throw new Error('workspace node process group survived cleanup')
await rm(pidfile, { force: true })

function required(name: string): string {
  const value = process.env[name]
  if (!value) throw new Error(`${name} is required`)
  return value
}

async function identity(target: number): Promise<{ pgid: number; startTime: string } | undefined> {
  if (process.platform === 'linux') try {
    const stat = await readFile(`/proc/${target}/stat`, 'utf8')
    const fields = stat.slice(stat.lastIndexOf(')') + 2).trim().split(/\s+/)
    return { pgid: Number(fields[2]), startTime: fields[19]! }
  } catch { return undefined }
  const result = Bun.spawnSync(['ps', '-p', String(target), '-o', 'pgid=', '-o', 'lstart='])
  if (result.exitCode !== 0) return undefined
  const match = result.stdout.toString().trim().match(/^(\d+)\s+(.+)$/)
  return match ? { pgid: Number(match[1]), startTime: match[2]! } : undefined
}

async function processExecutable(target: number): Promise<string | undefined> {
  if (process.platform === 'linux') {
    try { return await realpath(`/proc/${target}/exe`) } catch { return undefined }
  }
  const result = Bun.spawnSync(['lsof', '-a', '-p', String(target), '-d', 'txt', '-Fn'])
  if (result.exitCode !== 0) return undefined
  const path = result.stdout.toString().split('\n').find((line) => line.startsWith('n'))?.slice(1)
  if (!path) return undefined
  try { return await realpath(path) } catch { return undefined }
}

async function processUsesConfig(target: number, expected: string[]): Promise<boolean> {
  if (process.platform === 'linux') {
    const argv = (await readFile(`/proc/${target}/cmdline`, 'utf8')).split('\0').filter(Boolean)
    const index = argv.indexOf('--config')
    return index >= 0 && Boolean(argv[index + 1]) && expected.includes(await realpath(argv[index + 1]!))
  }
  const result = Bun.spawnSync(['ps', '-ww', '-p', String(target), '-o', 'command='])
  if (result.exitCode !== 0) return false
  const command = result.stdout.toString().trim()
  return expected.some((path) => {
    const escaped = path.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    return new RegExp(`(?:^|\\s)--config(?:\\s+|=)${escaped}(?=\\s|$)`).test(command)
  })
}

async function sameProcess(target: number, startTime: string): Promise<boolean> {
  return (await identity(target))?.startTime === startTime
}

function groupAlive(pgid: number): boolean {
  if (process.platform !== 'linux') {
    const result = Bun.spawnSync(['ps', '-axo', 'pgid=,state='])
    if (result.exitCode !== 0) throw new Error(`could not inspect process groups: ${result.stderr}`)
    return result.stdout.toString().split('\n').some((line) => {
      const match = line.trim().match(/^(\d+)\s+(\S+)/)
      return Number(match?.[1]) === pgid && !match?.[2]?.startsWith('Z')
    })
  }
  try { process.kill(-pgid, 0); return true } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ESRCH') return false
    throw error
  }
}

function signalGroup(pgid: number, signal: NodeJS.Signals): void {
  if (process.platform === 'darwin') {
    // macOS can briefly return EPERM while a child is already exiting. The
    // bounded waits and final groupAlive check below remain authoritative.
    Bun.spawnSync(['/bin/kill', `-${signal.replace(/^SIG/, '')}`, '--', `-${pgid}`])
    return
  }
  try { process.kill(-pgid, signal) } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ESRCH') throw error
  }
}

async function waitForExit(pgid: number, milliseconds: number): Promise<void> {
  const deadline = Date.now() + milliseconds
  while (groupAlive(pgid) && Date.now() < deadline) await Bun.sleep(50)
}
