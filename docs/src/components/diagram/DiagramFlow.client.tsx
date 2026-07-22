'use client'

import {
  Background,
  BackgroundVariant,
  Controls,
  type Edge,
  type Node,
  type NodeProps,
  Handle,
  Panel,
  Position,
  ReactFlow,
  ReactFlowProvider,
} from '@xyflow/react'
import { useEffect, useState, type CSSProperties } from 'react'
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

  const nodeLabels = new Map(
    nodes.map((node) => [node.id, typeof node.data.label === 'string' ? node.data.label : node.id]),
  )
  const mobileWidth = Math.min(1440, Math.max(720, ...nodes.map((node) => node.position.x + 240)))
  const canvasClass = mobileWidth > 720
    ? 'dt-diagram__canvas dt-diagram__canvas--wide'
    : 'dt-diagram__canvas'

  return (
    <figure className="dt-diagram" role="group" aria-label={title ?? description}>
      <div className="dt-diagram__frame">
        {mounted ? (
          <ReactFlowProvider>
            <div className="dt-diagram__scroll">
              <div
                className={canvasClass}
                style={{ height, '--dt-diagram-mobile-width': `${mobileWidth}px` } as CSSProperties}
              >
                <div className="dt-diagram__visual" aria-hidden="true" inert>
                  <ReactFlow
                    nodes={nodes}
                    edges={edges}
                    nodeTypes={nodeTypes}
                    fitView
                    fitViewOptions={{ padding: 0.16 }}
                    minZoom={0.2}
                    maxZoom={1.75}
                    nodesDraggable={draggable}
                    nodesConnectable={false}
                    nodesFocusable={false}
                    edgesFocusable={false}
                    elementsSelectable={false}
                    disableKeyboardA11y
                    zoomOnScroll={false}
                    zoomOnPinch={false}
                    panOnScroll={false}
                    panOnDrag={false}
                    preventScrolling={false}
                    zoomOnDoubleClick={false}
                    proOptions={{ hideAttribution: true }}
                  >
                    <Background variant={BackgroundVariant.Dots} gap={22} size={1} className="dt-bg" />
                  </ReactFlow>
                </div>
              </div>
            </div>
            <Controls
              aria-label={`${title ?? 'Diagram'} view controls`}
              fitViewOptions={{ padding: 0.16 }}
              showInteractive={false}
              position="bottom-right"
            />
            <Panel position="bottom-left" className="react-flow__attribution">
              <a
                href="https://reactflow.dev?utm_source=attribution"
                target="_blank"
                rel="noopener noreferrer"
                aria-label="React Flow attribution"
              >
                React Flow
              </a>
            </Panel>
          </ReactFlowProvider>
        ) : (
          <div className="dt-diagram__scroll">
            <div
              className={canvasClass}
              style={{ height, '--dt-diagram-mobile-width': `${mobileWidth}px` } as CSSProperties}
            >
              <div className="dt-diagram__skeleton" aria-hidden="true" />
            </div>
          </div>
        )}
      </div>
      <div className="dt-diagram__alternative">
        <p>Nodes</p>
        <ul>
          {nodes.map((node) => (
            <li key={node.id}>
              {nodeLabels.get(node.id)}
              {typeof node.data.sub === 'string' ? ` — ${node.data.sub}` : null}
            </li>
          ))}
        </ul>
        <p>Connections</p>
        <ul>
          {edges.map((edge) => (
            <li key={edge.id}>
              {nodeLabels.get(edge.source) ?? edge.source} to {nodeLabels.get(edge.target) ?? edge.target}
              {typeof edge.label === 'string' ? ` — ${edge.label}` : null}
            </li>
          ))}
        </ul>
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
