'use client'

import dagre from '@dagrejs/dagre'
import { MarkerType } from '@xyflow/react'
import { DiagramFlow, type DiagramEdge, type DiagramNode, type LegendItem } from './DiagramFlow.client'

/**
 * GraphFlow — renders a *semantic* graph (categorized nodes + typed edges) onto
 * the themed DiagramFlow surface, auto-laying it out with dagre. Diagram modules
 * describe *what* connects to *what*; layout is derived, so the data stays close
 * to the codebase ground truth without hand-placing coordinates.
 */

export interface SemanticNode {
  id: string
  label: string
  sub?: string
  category:
    | 'kernel'
    | 'system'
    | 'app'
    | 'duckfs'
    | 'bin'
    | 'external'
    | 'actor'
    | 'network'
    | 'store'
    | 'step'
}

export interface SemanticEdge {
  from: string
  to: string
  label?: string
  kind?: string
}

const CATEGORY_TONE: Record<SemanticNode['category'], string> = {
  kernel: 'kernel',
  system: 'system',
  app: 'app',
  duckfs: 'duckfs',
  bin: 'external',
  external: 'external',
  actor: 'actor',
  network: 'network',
  store: 'store',
  step: 'accent',
}

function estWidth(n: SemanticNode): number {
  const label = n.label.length * 8 + 34
  const sub = (n.sub?.length ?? 0) * 5.7 + 26
  return Math.min(238, Math.max(150, label, sub))
}

export interface GraphFlowProps {
  nodes: SemanticNode[]
  edges: SemanticEdge[]
  direction?: 'TB' | 'LR'
  height?: number
  ranksep?: number
  nodesep?: number
  legend?: LegendItem[]
  title?: string
  description?: string
  /** Edge `kind`s to render animated (a moving dashed flow). */
  animatedKinds?: string[]
}

export function GraphFlow({
  nodes,
  edges,
  direction = 'TB',
  height = 520,
  ranksep = 62,
  nodesep = 34,
  legend,
  title,
  description,
  animatedKinds = [],
}: GraphFlowProps) {
  const g = new dagre.graphlib.Graph()
  g.setDefaultEdgeLabel(() => ({}))
  g.setGraph({ rankdir: direction, ranksep, nodesep, marginx: 14, marginy: 14, ranker: 'network-simplex' })

  const sized = nodes.map((n) => ({ ...n, w: estWidth(n), h: n.sub ? 58 : 42 }))
  sized.forEach((n) => g.setNode(n.id, { width: n.w, height: n.h }))
  edges.forEach((e) => g.setEdge(e.from, e.to))
  dagre.layout(g)

  const isLR = direction === 'LR'
  const handles = isLR
    ? ({ left: 'target', right: 'source' } as const)
    : ({ top: 'target', bottom: 'source' } as const)
  const sh = isLR ? 'right' : 'bottom'
  const th = isLR ? 'left' : 'top'

  const rfNodes: DiagramNode[] = sized.map((n) => {
    const p = g.node(n.id)
    return {
      id: n.id,
      type: 'card',
      position: { x: p.x - n.w / 2, y: p.y - n.h / 2 },
      data: { label: n.label, sub: n.sub, tone: CATEGORY_TONE[n.category], handles },
    }
  })

  const rfEdges: DiagramEdge[] = edges.map((e, i) => ({
    id: `e${i}-${e.from}-${e.to}`,
    source: e.from,
    target: e.to,
    sourceHandle: sh,
    targetHandle: th,
    label: e.label,
    animated: e.kind ? animatedKinds.includes(e.kind) : false,
    type: 'smoothstep',
    markerEnd: { type: MarkerType.ArrowClosed, width: 14, height: 14 },
  }))

  return (
    <DiagramFlow
      title={title}
      description={description}
      nodes={rfNodes}
      edges={rfEdges}
      legend={legend}
      height={height}
    />
  )
}
