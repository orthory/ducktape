'use client'

import { GraphFlow, type SemanticEdge, type SemanticNode } from './GraphFlow.client'

/**
 * From submitted op to committed block, as a horizontal pipeline: signed op
 * frames batch on a ~2s cadence into a super-frame, Simplex agrees the batch,
 * the node applies it as one block, the app-hash is sealed, and effects leave
 * the hot path and re-enter later as ordinary ordered ops. Curated from the
 * consensus / node / saga ground truth.
 */

const nodes: SemanticNode[] = [
  { id: 'frames', label: 'Signed op frames', sub: 'member-authored', category: 'network' },
  { id: 'batch', label: 'Batch super-frame', sub: '~2s cadence', category: 'network' },
  { id: 'simplex', label: 'Simplex agree', sub: 'BFT view + finalize', category: 'kernel' },
  { id: 'drain', label: 'Node drain', sub: 'apply as one block', category: 'kernel' },
  { id: 'apphash', label: 'App-hash + seal', sub: 'modules execute · roots', category: 'store' },
  { id: 'effects', label: 'Effects re-enter', sub: 'saga → reactor → op', category: 'system' },
]

const edges: SemanticEdge[] = [
  { from: 'frames', to: 'batch', label: 'enqueue' },
  { from: 'batch', to: 'simplex', label: 'order', kind: 'order' },
  { from: 'simplex', to: 'drain', label: 'deliver', kind: 'order' },
  { from: 'drain', to: 'apphash', label: 'commit', kind: 'commit' },
  { from: 'apphash', to: 'effects', label: 'emit' },
  { from: 'effects', to: 'frames', label: 're-enter' },
]

const legend = [
  { color: '#9a9df0', label: 'Ordered lane' },
  { color: '#8b93f0', label: 'Consensus (Simplex)' },
  { color: '#d986d2', label: 'Committed root' },
  { color: '#4fd0b6', label: 'Effect seam' },
]

export function ConsensusFlow({ height = 340 }: { height?: number }) {
  return (
    <GraphFlow
      title="Consensus & block commit"
      description="Signed op frames batch on a ~2s cadence into a super-frame that Simplex agrees; the node applies the finalized batch as one block with an agreed height and time, seals the app-hash, and lets effects rejoin later as ordinary ordered ops."
      nodes={nodes}
      edges={edges}
      legend={legend}
      animatedKinds={['order', 'commit']}
      height={height}
      direction="LR"
      ranksep={72}
      nodesep={40}
    />
  )
}
