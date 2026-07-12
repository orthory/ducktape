import { expect, test } from 'bun:test'
import { chmod, mkdtemp, mkdir, readFile, realpath, rm, stat, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const root = new URL('../..', import.meta.url).pathname

test('Fleet config is CEF-only and points at owned hooks', async () => {
  expect(await Bun.file(join(root, 'tauri-agent-fleet.json')).exists()).toBe(false)
  expect(await Bun.file(join(root, 'qa/suites/cef-smoke.json')).exists()).toBe(false)
  const config = await Bun.file(join(root, '.tauri-agent/fleet.json')).json()
  expect(config).toEqual({
    protocol: 'tauri-agent-fleet/v1',
    application: { id: 'com.ducktape.app', root: '.' },
    lifecycle: {
      prepareInstance: ['bash', 'qa/fleet/prepare-instance.sh'],
      cleanupInstance: ['bun', 'qa/fleet/cleanup-instance.ts']
    },
    runtimes: { default: 'cef', cef: { build: ['bash', 'qa/fleet/build-cef.sh'] } }
  })
  expect(Bun.spawnSync(['bash', '-n', join(root, 'qa/fleet/build-cef.sh')]).exitCode).toBe(0)
  const buildHook = await Bun.file(join(root, 'qa/fleet/build-cef.sh')).text()
  expect(buildHook).toContain('VITE_TAURI_AGENT=1')
  expect(buildHook).toContain('tauri-agent-artifact/v1')
  expect(buildHook).toContain('Darwin)')
  expect(buildHook).toContain('--bundles app')
  expect(buildHook).toContain('check-macos-cef-bundle.sh')
  expect(buildHook).toContain('app/Ducktape.app/Contents/MacOS/ducktape-desktop')
  expect(buildHook).toContain("artifact_cwd='app/Ducktape.app/Contents/MacOS'")
  expect(buildHook).toContain('artifact_env=\'{ "LD_LIBRARY_PATH": "." }\'')
  expect(await Bun.file(join(root, 'app/src/main.tsx')).text()).toContain(
    'import.meta.env.VITE_TAURI_AGENT === "1"'
  )
  const smoke = await Bun.file(join(root, '.tauri-agent/suites/cef-smoke.toon')).text()
  for (const expected of [
    'protocol: tauri-agent-suite/v1',
    'id: cef-smoke',
    'runtime: cef',
    'role: button',
    'name: Create account',
    'steps: 3',
    'seconds: 30',
    'tokens: 1000',
    'repetitions: 2'
  ]) expect(smoke).toContain(expected)
})

test('cleanup hook terminates only the recorded instance node group on this platform', async () => {
  const scratch = await mkdtemp(join(tmpdir(), 'ducktape-fleet-cleanup-'))
  const artifact = join(scratch, 'artifact')
  const home = join(scratch, 'home')
  const targetConfig = join(home, '.ducktape/workspaces/target/node.toml')
  const siblingConfig = join(home, '.ducktape/workspaces/sibling/node.toml')
  const node = join(artifact, 'bin/ducktape-node')
  const bundledNode = join(artifact, 'app/Ducktape.app/Contents/MacOS/ducktape-node')
  await Promise.all([
    mkdir(join(artifact, 'bin'), { recursive: true }),
    mkdir(join(artifact, 'app/Ducktape.app/Contents/MacOS'), { recursive: true }),
    mkdir(join(home, '.ducktape/workspaces/target'), { recursive: true }),
    mkdir(join(home, '.ducktape/workspaces/sibling'), { recursive: true })
  ])
  await Promise.all([symlink('/bin/bash', bundledNode), writeFile(targetConfig, ''), writeFile(siblingConfig, '')])
  await symlink('../app/Ducktape.app/Contents/MacOS/ducktape-node', node)
  expect(await realpath(node)).toBe(await realpath(bundledNode))
  const start = (config: string) => {
    const launcher = Bun.spawnSync(['bun', '-e', `
      const [node, config] = Bun.argv.slice(1)
      const child = Bun.spawn({ cmd: [node, '-c', 'trap "exit 0" TERM; while :; do sleep 1; done', 'ducktape-node', '--config', config], detached: true, stdin: 'ignore', stdout: 'ignore', stderr: 'ignore' })
      console.log(child.pid)
      child.unref()
    `, node, config])
    if (launcher.exitCode !== 0) throw new Error(launcher.stderr.toString())
    return Number(launcher.stdout.toString().trim())
  }
  const target = start(targetConfig)
  const sibling = start(siblingConfig)
  const prefixed = start(`${targetConfig}.evil`)
  const kill = (pid: number) => { try { process.kill(-pid, 'SIGKILL') } catch { /* already stopped */ } }
  try {
    await writeFile(join(home, '.ducktape/workspaces/target/node.pid'), String(prefixed))
    const rejected = Bun.spawnSync(['bun', join(root, 'qa/fleet/cleanup-instance.ts')], {
      cwd: root,
      env: { ...process.env, FLEET_ARTIFACT_DIR: artifact, FLEET_HOME: home, FLEET_INSTANCE_ID: 'target' }
    })
    expect(rejected.exitCode).not.toBe(0)
    expect(alive(prefixed)).toBe(true)

    await writeFile(join(home, '.ducktape/workspaces/target/node.pid'), String(target))
    const result = Bun.spawnSync(['bun', join(root, 'qa/fleet/cleanup-instance.ts')], {
      cwd: root,
      env: { ...process.env, FLEET_ARTIFACT_DIR: artifact, FLEET_HOME: home, FLEET_INSTANCE_ID: 'target' }
    })
    expect(result.exitCode, result.stderr.toString()).toBe(0)
    expect(alive(target)).toBe(false)
    expect(alive(sibling)).toBe(true)
    expect(alive(prefixed)).toBe(true)
    expect(await Bun.file(join(home, '.ducktape/workspaces/target/node.pid')).exists()).toBe(false)
    expect(Bun.spawnSync(['bun', join(root, 'qa/fleet/cleanup-instance.ts')], {
      cwd: root,
      env: { ...process.env, FLEET_ARTIFACT_DIR: artifact, FLEET_HOME: home, FLEET_INSTANCE_ID: 'target' }
    }).exitCode).toBe(0)
  } finally {
    kill(target)
    kill(sibling)
    kill(prefixed)
    await rm(scratch, { recursive: true, force: true })
  }
})

function alive(pid: number): boolean {
  try { process.kill(pid, 0); return true } catch { return false }
}

test('instance hook seeds a private isolated workspace from the artifact', async () => {
  const scratch = await mkdtemp(join(tmpdir(), 'ducktape-fleet-'))
  try {
    const artifact = join(scratch, 'artifact')
    const home = join(scratch, 'home')
    const node = join(artifact, 'bin', 'ducktape-node')
    await mkdir(join(artifact, 'bin'), { recursive: true })
    await mkdir(home)
    await writeFile(node, `#!/usr/bin/env bash
set -eu
case "$1" in
  init) echo test-chain ;;
  keygen) while [ "$1" != "--out" ]; do shift; done; touch "$2"; echo test-pubkey ;;
esac
`)
    await chmod(node, 0o700)
    const result = Bun.spawnSync(['bash', join(root, 'qa/fleet/prepare-instance.sh')], {
      cwd: root,
      env: { ...process.env, FLEET_ARTIFACT_DIR: artifact, FLEET_HOME: home, FLEET_INSTANCE_ID: 'smoke-1234' }
    })
    expect(result.exitCode, result.stderr.toString()).toBe(0)
    const registry = JSON.parse(await readFile(join(home, '.ducktape/registry.json'), 'utf8'))
    expect(registry.active).toBe('smoke-1234')
    expect(registry.workspaces[0]).toMatchObject({ id: 'smoke-1234', chainId: 'test-chain', pubkey: 'test-pubkey', founder: true, member: true })
    const ports = Object.values(registry.workspaces[0].ports) as number[]
    expect(new Set(ports).size).toBe(3)
    expect(ports.every((port) => Number.isInteger(port) && port > 0 && port <= 65_535)).toBe(true)
    expect((await stat(join(home, '.ducktape/registry.json'))).mode & 0o777).toBe(0o600)
    expect((await stat(join(home, '.ducktape/workspaces/smoke-1234'))).mode & 0o777).toBe(0o700)
  } finally { await rm(scratch, { recursive: true, force: true }) }
})
