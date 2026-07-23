'use client'

import { GraphFlow, type SemanticEdge, type SemanticNode } from './GraphFlow.client'

/**
 * The host-owned block lifecycle of a module: register → execute → drain
 * follow-ups → commit/abort → fold root-hash → serve finalized sync.
 * Grounded in the `sdk` Module contract and the host block loop.
 */

const nodes: SemanticNode[] = [
  { id: 'register', label: 'Register', sub: 'genesis module id', category: 'step' },
  { id: 'receive', label: 'Receive op', sub: 'agreed Msg + Env', category: 'step' },
  { id: 'execute', label: 'Execute + stage', sub: 'Module::execute', category: 'step' },
  { id: 'emit', label: 'Emit follow-up', sub: 'Ctx::emit_msg', category: 'step' },
  { id: 'drain', label: 'Drain FIFO', sub: 'follow-up queue', category: 'step' },
  { id: 'commit', label: 'commit_block', sub: 'publish staged writes', category: 'store' },
  { id: 'abort', label: 'abort_block', sub: 'reject / budget → rollback', category: 'external' },
  { id: 'roothash', label: 'Fold root-hash', sub: 'recompose global root', category: 'store' },
  { id: 'serve', label: 'Serve sync', sub: 'finalized boundary', category: 'step' },
]

const edges: SemanticEdge[] = [
  { from: 'register', to: 'receive', label: 'boot' },
  { from: 'receive', to: 'execute', label: 'dispatch' },
  { from: 'execute', to: 'emit', label: 'may emit' },
  { from: 'emit', to: 'drain', label: 'queues' },
  { from: 'execute', to: 'drain', label: 'records' },
  { from: 'drain', to: 'execute', label: 'next msg' },
  { from: 'drain', to: 'commit', label: 'clean', kind: 'commit' },
  { from: 'drain', to: 'abort', label: 'reject' },
  { from: 'commit', to: 'roothash', label: 'recompose', kind: 'commit' },
  { from: 'roothash', to: 'serve', label: 'finalize', kind: 'commit' },
]

const legend = [
  { color: '#e6a94a', label: 'Block step' },
  { color: '#d986d2', label: 'Committed state' },
  { color: '#9aa4b2', label: 'Abort / rollback' },
]

export function ModuleLifecycle({ height = 560 }: { height?: number }) {
  return (
    <GraphFlow
      title="Module block lifecycle"
      description="For each agreed op the host builds a deterministic Env/Ctx, calls execute, drains any follow-up messages in the same block, then commits every touched module in registry order — or aborts the whole block, leaving roots unchanged."
      nodes={nodes}
      edges={edges}
      legend={legend}
      animatedKinds={['commit']}
      height={height}
      ranksep={54}
    />
  )
}
