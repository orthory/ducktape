'use client'

import {
  Background,
  BackgroundVariant,
  Controls,
  type Edge,
  type Node,
  type NodeProps,
  Handle,
  Position,
  ReactFlow,
  ReactFlowProvider,
} from '@xyflow/react'
import { useEffect, useState } from 'react'
import '@xyflow/react/dist/style.css'
import './diagram.css'

/**
 * DiagramFlow — the shared, themed React Flow surface for every Ducktape docs
 * diagram. Data-driven: a diagram module supplies `nodes`/`edges` built from
 * the shared node types below, and this wrapper handles theming, SSG safety
 * (mount gate), non-scroll-hijacking interaction, the legend, and the caption.
 */

export type DiagramNode = Node
export type DiagramEdge = Edge

export type LegendItem = { color: string; label: string }

export interface DiagramFlowProps {
  title?: string
  /** Accessible description; also rendered as the figure caption. */
  description?: string
  nodes: DiagramNode[]
  edges: DiagramEdge[]
  /** Container height in px. */
  height?: number
  /** Allow dragging nodes. Default false for a clean, stable layout. */
  draggable?: boolean
  legend?: LegendItem[]
}

/* ----------------------------- custom node kinds ---------------------------- */

type Tone =
  | 'kernel'
  | 'system'
  | 'app'
  | 'external'
  | 'actor'
  | 'store'
  | 'accent'
  | 'muted'

interface CardData {
  label: string
  sub?: string
  tone?: Tone
  /** Which handles to expose. Default: top target + bottom source. */
  handles?: Partial<Record<'top' | 'bottom' | 'left' | 'right', 'source' | 'target'>>
  [key: string]: unknown
}

const handlePos = { top: Position.Top, bottom: Position.Bottom, left: Position.Left, right: Position.Right } as const

function CardNode({ data }: NodeProps) {
  const d = data as CardData
  const handles = d.handles ?? { top: 'target', bottom: 'source' }
  return (
    <div className={`dt-node dt-node--${d.tone ?? 'system'}`}>
      {Object.entries(handles).map(([pos, kind]) => (
        <Handle
          key={pos}
          id={pos}
          type={kind as 'source' | 'target'}
          position={handlePos[pos as keyof typeof handlePos]}
          className="dt-handle"
          isConnectable={false}
        />
      ))}
      <span className="dt-node__label">{d.label}</span>
      {d.sub ? <span className="dt-node__sub">{d.sub}</span> : null}
    </div>
  )
}

interface GroupData {
  label: string
  tone?: Tone
  [key: string]: unknown
}

function GroupNode({ data }: NodeProps) {
  const d = data as GroupData
  return (
    <div className={`dt-group dt-group--${d.tone ?? 'muted'}`}>
      <span className="dt-group__label">{d.label}</span>
    </div>
  )
}

export const nodeTypes = { card: CardNode, group: GroupNode }

/* -------------------------------- component -------------------------------- */

export function DiagramFlow({
  title,
  description,
  nodes,
  edges,
  height = 440,
  draggable = false,
  legend,
}: DiagramFlowProps) {
  // Mount gate: React Flow measures the DOM, so it must only run on the client.
  // Pre-mount we render an accessible skeleton (also the SSG/no-JS fallback).
  const [mounted, setMounted] = useState(false)
  useEffect(() => setMounted(true), [])

  return (
    <figure className="dt-diagram" role="group" aria-label={title ?? description}>
      <div className="dt-diagram__canvas" style={{ height }}>
        {mounted ? (
          <ReactFlowProvider>
            <ReactFlow
              nodes={nodes}
              edges={edges}
              nodeTypes={nodeTypes}
              fitView
              fitViewOptions={{ padding: 0.16 }}
              minZoom={0.35}
              maxZoom={1.75}
              nodesDraggable={draggable}
              nodesConnectable={false}
              elementsSelectable
              zoomOnScroll={false}
              panOnScroll={false}
              preventScrolling={false}
              zoomOnDoubleClick={false}
              proOptions={{ hideAttribution: false }}
            >
              <Background variant={BackgroundVariant.Dots} gap={22} size={1} className="dt-bg" />
              <Controls showInteractive={false} position="bottom-right" />
            </ReactFlow>
          </ReactFlowProvider>
        ) : (
          <div className="dt-diagram__skeleton" aria-hidden="true" />
        )}
      </div>
      {legend && legend.length > 0 ? (
        <ul className="dt-legend">
          {legend.map((item) => (
            <li key={item.label} className="dt-legend__item">
              <span className="dt-legend__swatch" style={{ background: item.color }} />
              {item.label}
            </li>
          ))}
        </ul>
      ) : null}
      {description ? <figcaption className="dt-diagram__caption">{description}</figcaption> : null}
    </figure>
  )
}
