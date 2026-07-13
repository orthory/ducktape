#!/usr/bin/env bun
/**
 * Drive the real macOS CEF app through sandbox Apply and rollback. Fleet owns
 * the throwaway HOME/processes; this driver asserts the durable config, node
 * PID transition, and HTTP recovery behind the visible UI result.
 */
import { readFile, writeFile } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const fleet = process.env.FLEET ?? join(root, 'app/node_modules/@byeongsu-hong/tauri-agent-fleet/dist/cli.js')
const agent = process.env.TAURI_AGENT ?? join(root, 'app/scripts/tauri-agent')
const password = 'fleet-sandbox-smoke-password'
const scenario = process.argv[2] ?? 'success'
if (process.platform !== 'darwin') throw new Error('macOS sandbox Apply smoke requires Darwin')
if (scenario !== 'success' && scenario !== 'rollback') throw new Error('usage: macos-sandbox-apply-smoke.ts [success|rollback]')

type FleetInstance = {
  id: string
  state: string
  directories: { home: string; runtime: string; artifacts: string }
}

async function command(
  argv: string[],
  options: { env?: Record<string, string | undefined>; visible?: boolean; allowFailure?: boolean } = {}
): Promise<string> {
  const child = Bun.spawn(argv, {
    cwd: root,
    env: options.env ?? process.env,
    stdin: 'ignore',
    stdout: options.visible ? 'inherit' : 'pipe',
    stderr: options.visible ? 'inherit' : 'pipe'
  })
  const stdout = options.visible ? '' : await new Response(child.stdout).text()
  const stderr = options.visible ? '' : await new Response(child.stderr).text()
  const exit = await child.exited
  if (exit !== 0 && !options.allowFailure) {
    throw new Error(`${argv.join(' ')} exited ${exit}${stderr ? `: ${stderr.trim()}` : ''}`)
  }
  return stdout.trim()
}

async function fleetStatus(env: Record<string, string | undefined>): Promise<FleetInstance[]> {
  return JSON.parse(await command([fleet, 'status', '--json'], { env })).instances
}

async function until<T>(label: string, fn: () => Promise<T | null>, timeoutMs = 45_000): Promise<T> {
  const deadline = Date.now() + timeoutMs
  let last: unknown
  while (Date.now() < deadline) {
    try {
      const value = await fn()
      if (value !== null) return value
    } catch (error) {
      last = error
    }
    await Bun.sleep(500)
  }
  throw new Error(`${label} timed out${last ? `: ${String(last)}` : ''}`)
}

let instance: FleetInstance | undefined
let env: Record<string, string | undefined> = {
  ...process.env,
  FLEET_QA_IDENTITY_PASSWORD: password
}
if (scenario === 'rollback') {
  env = {
    ...env,
    DUCKTAPE_NODE_BIN: join(root, 'qa/fleet/fail-next-node-start.sh')
  }
}
const prefix = `macos-sandbox-${scenario}-${process.pid}`
const before = new Set((await fleetStatus(env)).map((candidate) => candidate.id))

try {
  await command([fleet, 'up', 'HEAD', '--runtime', 'cef', '--id', prefix], { env, visible: true })
  instance = await until('Fleet instance readiness', async () => {
    const candidate = (await fleetStatus(env)).find((item) => !before.has(item.id) && item.id.startsWith(prefix))
    return candidate?.state === 'ready' ? candidate : null
  })
  const agentEnv = { ...process.env, XDG_RUNTIME_DIR: instance.directories.runtime }
  const agentCommand = (name: string, ...args: string[]) =>
    command([agent, name, ...args, '--app', 'com.ducktape.app'], { env: agentEnv })
  const waitText = (text: string, timeout = 60_000) => agentCommand('wait', text, '--timeout-ms', String(timeout))
  const ref = async (role: string, name: string): Promise<string> => {
    const result = JSON.parse(await agentCommand('find', '--role', role, '--name', name, '--limit', '50'))
    const found = result.matches?.find((match: { name?: string }) => match.name === name)?.ref
    if (!found) throw new Error(`no ${role} named ${JSON.stringify(name)}`)
    return found
  }
  const click = async (role: string, name: string) => agentCommand('click', await ref(role, name))

  await waitText('Unlock your account')
  await agentCommand('fill', await ref('textbox', 'Password'), password)
  await click('button', 'Unlock')
  await until('identity unlock', async () => {
    const result = JSON.parse(await agentCommand('find', '--role', 'tab', '--name', 'Node operator', '--limit', '1'))
    return result.matches?.length ? true : null
  })
  await click('tab', 'Node operator')
  await click('button', 'Sandbox')
  await waitText('Sandbox serving')

  const workspace = join(instance.directories.home, '.ducktape/workspaces', instance.id)
  const configPath = join(workspace, 'node.toml')
  const pidPath = join(workspace, 'node.pid')
  const registry = JSON.parse(await readFile(join(instance.directories.home, '.ducktape/registry.json'), 'utf8'))
  const httpPort = registry.workspaces.find((item: { id: string }) => item.id === instance!.id).ports.http as number
  const nodePid = () => readFile(pidPath, 'utf8').then((text) => Number(text.trim()))
  const initialConfig = await readFile(configPath, 'utf8')
  let priorPid = await nodePid()

  const apply = async (mode: 'podman' | 'tart', expectRollback: boolean) => {
    const label = mode === 'podman' ? 'Podman' : 'Tart'
    await click('button', label)
    await waitText(`Apply ${label}?`)
    if (expectRollback) {
      await writeFile(join(instance!.directories.home, '.ducktape/qa-fail-next-node-start'), 'armed\n')
    }
    await click('button', 'Apply and restart')
    await waitText(expectRollback ? 'Apply failed:' : 'Applied. The node restarted', 90_000)

    await until('node HTTP recovery', async () => {
      try {
        const response = await fetch(`http://127.0.0.1:${httpPort}/v1/status`)
        return response.ok ? true : null
      } catch {
        return null
      }
    }, 30_000)
    const nextPid = await nodePid()
    if (!Number.isInteger(nextPid) || nextPid === priorPid) throw new Error(`node PID did not change (${priorPid} -> ${nextPid})`)
    priorPid = nextPid
    const config = await readFile(configPath, 'utf8')
    if (expectRollback) {
      if (config !== initialConfig) throw new Error('failed Apply did not restore node.toml byte-for-byte')
    } else {
      if (!/^announce_capabilities\s*=\s*true$/m.test(config) || !new RegExp(`^sandbox\\s*=\\s*"${mode}"$`, 'm').test(config)) {
        throw new Error(`Apply did not persist ${mode} in node.toml`)
      }
    }
    console.log(`[sandbox-ui] ${expectRollback ? 'rollback' : mode} passed; node ${nextPid}, HTTP ${httpPort}`)
  }

  if (scenario === 'rollback') {
    await apply('podman', true)
  } else {
    await apply('podman', false)
    await apply('tart', false)
  }
} catch (error) {
  if (instance) {
    await command(
      [agent, 'shot', join(instance.directories.artifacts, `${scenario}-failure.png`), '--app', 'com.ducktape.app'],
      { env: { ...process.env, XDG_RUNTIME_DIR: instance.directories.runtime }, allowFailure: true }
    )
  }
  throw error
} finally {
  const candidates = instance ? [instance] : (await fleetStatus(env)).filter((item) => !before.has(item.id) && item.id.startsWith(prefix))
  for (const candidate of candidates) {
    // A failed macOS spawn can be reported during exec; let the recorded app
    // finish exec before Fleet performs its second, identity-checked teardown.
    await Bun.sleep(2_000)
    await command([fleet, 'down', candidate.id], { env, visible: true, allowFailure: false })
  }
}
