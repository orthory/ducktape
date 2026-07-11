import { readFile, realpath, rm } from 'node:fs/promises'
import { join } from 'node:path'

const home = required('FLEET_HOME')
const id = required('FLEET_INSTANCE_ID')
const workspace = join(home, '.ducktape', 'workspaces', id)
const pidfile = join(workspace, 'node.pid')

let pid: number
try { pid = Number((await readFile(pidfile, 'utf8')).trim()) } catch { process.exit(0) }
if (!Number.isSafeInteger(pid) || pid <= 1) throw new Error('workspace node pidfile is invalid')

const expectedExecutable = await realpath(join(required('FLEET_ARTIFACT_DIR'), 'bin', 'ducktape-node'))
const config = await realpath(join(workspace, 'node.toml'))
const owned = await identity(pid)
if (!owned) { await rm(pidfile, { force: true }); process.exit(0) }
if (owned.pgid !== pid) throw new Error('workspace node is not the leader of its process group')
const executable = await realpath(`/proc/${pid}/exe`)
const argv = (await readFile(`/proc/${pid}/cmdline`, 'utf8')).split('\0').filter(Boolean)
const configIndex = argv.indexOf('--config')
if (executable !== expectedExecutable || configIndex < 0 || !argv[configIndex + 1] || await realpath(argv[configIndex + 1]!) !== config) {
  throw new Error('workspace node pidfile does not identify this Fleet instance')
}

try {
  const registry = JSON.parse(await readFile(join(home, '.ducktape', 'registry.json'), 'utf8')) as {
    workspaces?: Array<{ id?: string; ports?: { http?: number } }>
  }
  const port = registry.workspaces?.find((item) => item.id === id)?.ports?.http
  if (Number.isInteger(port) && port! > 0 && port! <= 65_535) {
    await fetch(`http://127.0.0.1:${port}/v1/shutdown`, { method: 'POST', signal: AbortSignal.timeout(500) }).catch(() => undefined)
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
  try {
    const stat = await readFile(`/proc/${target}/stat`, 'utf8')
    const fields = stat.slice(stat.lastIndexOf(')') + 2).trim().split(/\s+/)
    return { pgid: Number(fields[2]), startTime: fields[19]! }
  } catch { return undefined }
}

async function sameProcess(target: number, startTime: string): Promise<boolean> {
  return (await identity(target))?.startTime === startTime
}

function groupAlive(pgid: number): boolean {
  try { process.kill(-pgid, 0); return true } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ESRCH') return false
    throw error
  }
}

function signalGroup(pgid: number, signal: NodeJS.Signals): void {
  try { process.kill(-pgid, signal) } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ESRCH') throw error
  }
}

async function waitForExit(pgid: number, milliseconds: number): Promise<void> {
  const deadline = Date.now() + milliseconds
  while (groupAlive(pgid) && Date.now() < deadline) await Bun.sleep(50)
}
