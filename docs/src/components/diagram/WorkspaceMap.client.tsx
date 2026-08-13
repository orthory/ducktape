'use client'

import { GraphFlow, type SemanticEdge, type SemanticNode } from './GraphFlow.client'

/**
 * The Cargo workspace by layer: runnable bins on top, the kernel platform,
 * consensus-infrastructure system modules, product app modules, and the duckfs
 * stack — with the dependency direction that enforces the module-isolation rule
 * (apps and system modules point only at `sdk`). Curated from the workspace
 * dependency graph.
 */

const nodes: SemanticNode[] = [
  // bins
  { id: 'bin-node', label: 'bin/node', sub: 'validator process', category: 'bin' },
  { id: 'bin-coord', label: 'bin/coordinator', sub: 'UDP rendezvous', category: 'bin' },
  { id: 'cli-fs', label: 'ducktape fs', sub: 'duckfs CLI', category: 'bin' },
  // kernel
  { id: 'sdk', label: 'sdk', sub: 'module contract', category: 'kernel' },
  { id: 'host', label: 'host', sub: 'dispatch + root-hash + worker seam', category: 'kernel' },
  { id: 'node', label: 'node', sub: 'ordered path', category: 'kernel' },
  { id: 'consensus', label: 'consensus', sub: 'Simplex orderer', category: 'kernel' },
  { id: 'statesync', label: 'statesync + recovery', sub: 'rebuild + durability', category: 'kernel' },
  // system
  { id: 'valset', label: 'valset', sub: 'validator membership', category: 'system' },
  { id: 'saga', label: 'saga', sub: 'async continuations', category: 'system' },
  { id: 'dispatch', label: 'dispatch', sub: 'task plane', category: 'system' },
  { id: 'sysmore', label: '+15 system modules', sub: 'identity · governance · net …', category: 'system' },
  // apps
  { id: 'apps', label: '12 product modules', sub: 'chat · pages · forge · agent …', category: 'app' },
  { id: 'files', label: 'files', sub: 'duckfs module adapter', category: 'app' },
  // duckfs
  { id: 'duckfs', label: 'duckfs core', sub: 'CoW replicated FS', category: 'duckfs' },
]

const edges: SemanticEdge[] = [
  { from: 'bin-node', to: 'host', label: 'runs' },
  { from: 'bin-node', to: 'consensus', label: 'orders' },
  { from: 'cli-fs', to: 'duckfs', label: 'drives' },
  { from: 'apps', to: 'sdk', label: 'depends on', kind: 'dep' },
  { from: 'files', to: 'sdk', label: 'depends on', kind: 'dep' },
  { from: 'files', to: 'duckfs', label: 'adapts' },
  { from: 'valset', to: 'sdk', label: 'depends on', kind: 'dep' },
  { from: 'saga', to: 'sdk', label: 'depends on', kind: 'dep' },
  { from: 'dispatch', to: 'sdk', label: 'depends on', kind: 'dep' },
  { from: 'sysmore', to: 'sdk', label: 'depends on', kind: 'dep' },
  { from: 'host', to: 'sdk', label: 'contract' },
  { from: 'consensus', to: 'node', label: 'orders' },
  { from: 'node', to: 'host', label: 'applies' },
  { from: 'statesync', to: 'node', label: 'serves' },
]

const legend = [
  { color: '#9aa4b2', label: 'Runnable bin' },
  { color: '#8b93f0', label: 'Kernel' },
  { color: '#4fd0b6', label: 'System module' },
  { color: '#e6a94a', label: 'Product module' },
  { color: '#56bde8', label: 'duckfs' },
]

export function WorkspaceMap({ height = 620 }: { height?: number }) {
  return (
    <GraphFlow
      title="Cargo workspace by layer"
      description="Every product and system module depends only on sdk (and the types-only interface crates it addresses). The kernel wires the consensus → node → host path, and host composes the global root-hash; bins compose it into runnable processes."
      nodes={nodes}
      edges={edges}
      legend={legend}
      animatedKinds={['dep']}
      height={height}
      ranksep={66}
      nodesep={28}
    />
  )
}
