'use client'

import { GraphFlow, type SemanticEdge, type SemanticNode } from './GraphFlow.client'

/**
 * A joiner rebuilds committed state from an untrusted serving validator: fetch a
 * boundary manifest, stream each module through its declared sync surface,
 * recompose the app-hash, adopt only if it matches, then catch up the frame gap.
 * Curated from the statesync/recovery ground truth.
 */

const nodes: SemanticNode[] = [
  { id: 'source', label: 'Source validator', sub: 'serves finalized state', category: 'actor' },
  { id: 'boundary', label: 'Committed boundary', sub: 'height + app-hash', category: 'step' },
  { id: 'manifest', label: 'Boundary manifest', sub: 'roots + sync surfaces', category: 'step' },
  { id: 'joiner', label: 'Joiner', sub: 'fresh local state', category: 'actor' },
  { id: 'qmdb', label: 'QMDB lane', sub: 'proof op ranges', category: 'store' },
  { id: 'snapshot', label: 'Snapshot lane', sub: 'chunked bytes', category: 'store' },
  { id: 'forge', label: 'Forge / duckfs lane', sub: 'versioned install', category: 'duckfs' },
  { id: 'rebuild', label: 'Rebuild host', sub: 'module registry', category: 'kernel' },
  { id: 'check', label: 'App-hash check', sub: 'compose roots', category: 'step' },
  { id: 'adopt', label: 'Adopt state', sub: 'promote scratch', category: 'step' },
  { id: 'catchup', label: 'Frame catch-up', sub: 'post-boundary gap', category: 'step' },
  { id: 'live', label: 'Validator live', sub: 'votes after sync', category: 'actor' },
]

const edges: SemanticEdge[] = [
  { from: 'source', to: 'boundary', label: 'finalizes', kind: 'sync' },
  { from: 'boundary', to: 'manifest', label: 'captures' },
  { from: 'joiner', to: 'manifest', label: 'fetches', kind: 'sync' },
  { from: 'manifest', to: 'qmdb', label: 'pins' },
  { from: 'manifest', to: 'snapshot', label: 'lists' },
  { from: 'manifest', to: 'forge', label: 'roots' },
  { from: 'qmdb', to: 'rebuild', label: 'installs' },
  { from: 'snapshot', to: 'rebuild', label: 'installs' },
  { from: 'forge', to: 'rebuild', label: 'installs' },
  { from: 'rebuild', to: 'check', label: 'composes' },
  { from: 'check', to: 'adopt', label: 'gates', kind: 'sync' },
  { from: 'adopt', to: 'catchup', label: 'preflights' },
  { from: 'catchup', to: 'live', label: 'lands on tip', kind: 'sync' },
]

const legend = [
  { color: '#67ade6', label: 'Actor' },
  { color: '#d986d2', label: 'Sync surface' },
  { color: '#56bde8', label: 'Forge / duckfs' },
  { color: '#e6a94a', label: 'Verify step' },
]

export function StateSyncFlow({ height = 600 }: { height?: number }) {
  return (
    <GraphFlow
      title="State sync — joiner rebuild"
      description="The joiner streams every module through its declared sync surface, recomposes the global app-hash, and adopts the state only if it equals the source's committed root — then catches up the post-boundary frame gap before it votes."
      nodes={nodes}
      edges={edges}
      legend={legend}
      animatedKinds={['sync']}
      height={height}
      ranksep={54}
    />
  )
}
