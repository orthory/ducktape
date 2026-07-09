'use client'

import { GraphFlow, type SemanticEdge, type SemanticNode } from './GraphFlow.client'

/**
 * The async seam as a horizontal loop: how non-deterministic work (LLM calls,
 * network, time) leaves the deterministic hot path and returns without breaking
 * replay. A committed op emits an Effect; saga records a deterministic
 * continuation; the reactor runs the messy work off-path; the result re-enters
 * as an ordinary ordered op that every validator replays identically.
 */

const nodes: SemanticNode[] = [
  { id: 'op', label: 'Committed op', sub: 'deterministic execute', category: 'app' },
  { id: 'effect', label: 'Effect intent', sub: 'ask host for work', category: 'step' },
  { id: 'saga', label: 'saga', sub: 'continuation ledger', category: 'system' },
  { id: 'reactor', label: 'reactor', sub: 'worker loop, off-path', category: 'kernel' },
  { id: 'world', label: 'Non-det work', sub: 'LLM · network · time', category: 'external' },
  { id: 'result', label: 'Result op', sub: 'signed · ordered · replayed', category: 'store' },
]

const edges: SemanticEdge[] = [
  { from: 'op', to: 'effect', label: 'emits' },
  { from: 'effect', to: 'saga', label: 'records', kind: 'effect' },
  { from: 'saga', to: 'reactor', label: 'schedules', kind: 'effect' },
  { from: 'reactor', to: 'world', label: 'performs' },
  { from: 'world', to: 'result', label: 'produces' },
  { from: 'result', to: 'op', label: 're-enters ordered', kind: 'effect' },
]

const legend = [
  { color: '#e6a94a', label: 'Effect boundary' },
  { color: '#4fd0b6', label: 'saga (deterministic)' },
  { color: '#8b93f0', label: 'reactor (off-path)' },
  { color: '#9aa4b2', label: 'Outside world' },
]

export function AsyncEngine({ height = 340 }: { height?: number }) {
  return (
    <GraphFlow
      title="Async engine — effects without breaking replay"
      description="Modules never do non-deterministic work inline. They emit an Effect; saga records a deterministic continuation and the reactor performs the messy work off the hot path; the result re-enters as an ordinary ordered op that every validator replays to the same state."
      nodes={nodes}
      edges={edges}
      legend={legend}
      animatedKinds={['effect']}
      height={height}
      direction="LR"
      ranksep={72}
      nodesep={40}
    />
  )
}
