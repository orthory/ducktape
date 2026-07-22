'use client'

import { MarkerType } from '@xyflow/react'
import { DiagramFlow, type DiagramEdge, type DiagramNode } from './DiagramFlow.client'

/**
 * Role-level entry topology: NATed joiners dial OUT to an out-of-path rendezvous
 * coordinator or an in-path sentry front; the validator-owned userspace
 * WireGuard overlay then carries mesh + state-sync traffic. No machine
 * identifiers — roles only. Grounded in the deploy runbooks + net crates.
 */

const card = (
  id: string,
  x: number,
  y: number,
  label: string,
  tone: string,
  sub: string,
  handles: Record<string, 'source' | 'target'>,
): DiagramNode => ({ id, type: 'card', position: { x, y }, data: { label, sub, tone, handles } })

const nodes: DiagramNode[] = [
  card('joiners', 0, 118, 'NATed joiners', 'actor', 'outbound-only, no ports', { right: 'source', bottom: 'source' }),
  card('coordinator', 250, 8, 'Coordinator', 'network', 'rendezvous · out of path', { left: 'target', right: 'source' }),
  card('sentry', 250, 220, 'Sentry front', 'network', 'reverse tunnel · in path', { left: 'target', right: 'source' }),
  card('overlay', 495, 116, 'WireGuard overlay', 'network', 'userspace mesh backbone', { left: 'target', right: 'source', bottom: 'source' }),
  card('validators', 745, 8, 'Validators', 'actor', 'BFT quorum set', { left: 'target', bottom: 'source' }),
  card('residents', 745, 220, 'Residents', 'actor', 'mesh + state-sync, no vote', { left: 'target' }),
  card('statesync', 430, 300, 'State sync', 'system', 'validator-served rebuild', { top: 'target' }),
]

const edge = (
  id: string,
  source: string,
  target: string,
  sh: string,
  th: string,
  label: string,
  animated = false,
): DiagramEdge => ({
  id,
  source,
  target,
  sourceHandle: sh,
  targetHandle: th,
  label,
  animated,
  type: 'smoothstep',
  markerEnd: { type: MarkerType.ArrowClosed, width: 15, height: 15 },
})

const edges: DiagramEdge[] = [
  edge('j-c', 'joiners', 'coordinator', 'right', 'left', 'dial out'),
  edge('j-s', 'joiners', 'sentry', 'right', 'left', 'dial front'),
  edge('c-o', 'coordinator', 'overlay', 'right', 'left', 'punch'),
  edge('s-o', 'sentry', 'overlay', 'right', 'left', 'splice'),
  edge('o-v', 'overlay', 'validators', 'right', 'left', 'carry mesh', true),
  edge('o-r', 'overlay', 'residents', 'right', 'left', 'carry mesh', true),
  edge('v-ss', 'validators', 'statesync', 'bottom', 'top', 'serve'),
  edge('j-ss', 'joiners', 'statesync', 'bottom', 'top', 'state sync'),
]

const legend = [
  { color: '#67ade6', label: 'Node role' },
  { color: '#9a9df0', label: 'Network path' },
  { color: '#4fd0b6', label: 'State sync' },
]

export function NetworkTopology({ height = 470 }: { height?: number }) {
  return (
    <DiagramFlow
      title="Network entry topology"
      description="The coordinator only helps peers discover and punch — it never carries data. Sentries front fronted validator traffic. A joiner reaches the mesh with zero inbound ports, gains resident standing, and only becomes a validator through a separate promotion."
      nodes={nodes}
      edges={edges}
      legend={legend}
      height={height}
    />
  )
}
