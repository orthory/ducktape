import { expect, test } from 'bun:test'
import { chmod, mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const root = new URL('../..', import.meta.url).pathname

test('Fleet config is CEF-only and points at owned hooks', async () => {
  const config = await Bun.file(join(root, 'tauri-agent-fleet.json')).json()
  expect(config).toEqual({
    schemaVersion: 1,
    baseBranch: 'dev',
    projectDir: '.',
    agent: { appId: 'com.ducktape.app' },
    hooks: { prepareInstance: ['bash', 'qa/fleet/prepare-instance.sh'] },
    variants: { cef: { build: ['bash', 'qa/fleet/build-cef.sh'] } }
  })
  expect(Bun.spawnSync(['bash', '-n', join(root, 'qa/fleet/build-cef.sh')]).exitCode).toBe(0)
  expect(await Bun.file(join(root, 'qa/fleet/build-cef.sh')).text()).toContain('VITE_TAURI_AGENT=1')
  expect(await Bun.file(join(root, 'app/src/main.tsx')).text()).toContain(
    'import.meta.env.VITE_TAURI_AGENT === "1"'
  )
  const smoke = await Bun.file(join(root, 'qa/suites/cef-smoke.json')).json()
  expect(smoke.success).toEqual([{ expect: { role: 'button', name: 'Create account', present: true } }])
})

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
    expect(new Set(Object.values(registry.workspaces[0].ports)).size).toBe(3)
  } finally { await rm(scratch, { recursive: true, force: true }) }
})
