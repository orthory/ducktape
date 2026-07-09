'use client'

import { MarkerType } from '@xyflow/react'
import { DiagramFlow, type DiagramEdge, type DiagramNode } from './DiagramFlow.client'

/**
 * The Ducktape platform stack: how product/system modules, the host, the
 * ordered node path, and the Simplex orderer compose one BFT-replicated state
 * machine. Hand-positioned for a deliberate, legible layout.
 */

const card = (
  id: string,
  x: number,
  y: number,
  label: string,
  tone: string,
  sub?: string,
  handles?: Record<string, 'source' | 'target'>,
): DiagramNode => ({
  id,
  type: 'card',
  position: { x, y },
  data: { label, sub, tone, handles },
})

const nodes: DiagramNode[] = [
  card('modules', 40, 0, 'Product modules', 'app', 'chat · pages · forge · agent', { bottom: 'source' }),
  card('system', 360, 0, 'System modules', 'system', 'valset · governance · identity', { bottom: 'source' }),
  card('sdk', 205, 96, 'sdk — contract', 'kernel', 'Module · Msg · Effect · root', { top: 'target', bottom: 'source' }),
  card('reactor', 0, 200, 'reactor', 'kernel', 'effects, off hot path', { right: 'source' }),
  card('host', 205, 200, 'host', 'kernel', 'routes ops · folds roots', {
    top: 'target',
    bottom: 'source',
    right: 'source',
    left: 'target',
  }),
  card('state', 430, 200, 'state — QMDB', 'store', 'authenticated roots', { left: 'target' }),
  card('node', 205, 300, 'node', 'kernel', 'apply · commit blocks', { top: 'target', bottom: 'source' }),
  card('consensus', 150, 400, 'consensus — Simplex', 'kernel', 'BFT · height + time', { top: 'target' }),
  card('statesync', 430, 300, 'statesync + recovery', 'kernel', 'rebuild · durability', { left: 'target' }),
]

const edge = (
  id: string,
  source: string,
  target: string,
  label?: string,
  animated = false,
): DiagramEdge => ({
  id,
  source,
  target,
  label,
  animated,
  type: 'smoothstep',
  markerEnd: { type: MarkerType.ArrowClosed, width: 15, height: 15 },
})

const edges: DiagramEdge[] = [
  edge('m-sdk', 'modules', 'sdk', 'depends on'),
  edge('s-sdk', 'system', 'sdk', 'depends on'),
  edge('sdk-host', 'sdk', 'host', 'contract'),
  edge('host-state', 'host', 'state', 'read / stage'),
  edge('reactor-host', 'reactor', 'host', 'follow-ups'),
  edge('host-node', 'host', 'node', 'app-hash', true),
  edge('node-consensus', 'node', 'consensus', 'order', true),
  edge('node-sync', 'node', 'statesync', 'serve'),
]

const legend = [
  { color: '#8b93f0', label: 'Kernel' },
  { color: '#4fd0b6', label: 'System modules' },
  { color: '#e6a94a', label: 'Product modules' },
  { color: '#d986d2', label: 'State substrate' },
]

export function ArchitectureStack({ height = 480 }: { height?: number }) {
  return (
    <DiagramFlow
      title="Ducktape platform stack"
      description="Product and system modules depend only on the sdk contract. The host routes ordered ops, folds each module root into the global app-hash, and drives commit through the ordered node path under the Simplex BFT orderer."
      nodes={nodes}
      edges={edges}
      legend={legend}
      height={height}
    />
  )
}
