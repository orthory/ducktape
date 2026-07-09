'use client'

import { GraphFlow, type SemanticEdge, type SemanticNode } from './GraphFlow.client'

/**
 * Product modules and their *real* cross-module interactions — every edge is a
 * host-drained follow-up Msg or a host-routed read discovered through a
 * types-only interface crate. Most modules are self-contained stores; the live
 * collaboration loop clusters around runs, dispatch, and tagging. Grounded in
 * the apps/* interface deps.
 */

const nodes: SemanticNode[] = [
  { id: 'runs', label: 'runs', sub: 'agent runner', category: 'app' },
  { id: 'agent', label: 'agent', sub: 'agent registry', category: 'app' },
  { id: 'chat', label: 'chat', sub: 'channels + threads', category: 'app' },
  { id: 'automations', label: 'automations', sub: 'chat-hook rules', category: 'app' },
  { id: 'tasks', label: 'tasks', sub: 'task list', category: 'app' },
  { id: 'jobs', label: 'jobs', sub: 'work board', category: 'app' },
  { id: 'inbox', label: 'inbox', sub: 'notification queues', category: 'app' },
  { id: 'forge', label: 'forge', sub: 'git forge', category: 'app' },
  { id: 'dispatch', label: 'dispatch', sub: 'task plane', category: 'system' },
  { id: 'tagging', label: 'tagging', sub: 'engagement router', category: 'system' },
  { id: 'pages', label: 'pages', sub: 'block tree · self-contained', category: 'app' },
  { id: 'files', label: 'files', sub: 'duckfs adapter · self-contained', category: 'app' },
  { id: 'profiles', label: 'profiles', sub: 'display names · self-contained', category: 'app' },
  { id: 'vaults', label: 'vaults', sub: 'encrypted · self-contained', category: 'app' },
]

const edges: SemanticEdge[] = [
  { from: 'chat', to: 'tagging', label: 'reports tags', kind: 'message' },
  { from: 'tagging', to: 'runs', label: 'delivers engagement', kind: 'message' },
  { from: 'runs', to: 'dispatch', label: 'dispatches', kind: 'message' },
  { from: 'dispatch', to: 'runs', label: 'result', kind: 'message' },
  { from: 'runs', to: 'agent', label: 'reads registry', kind: 'read' },
  { from: 'runs', to: 'chat', label: 'reads + posts', kind: 'message' },
  { from: 'runs', to: 'tasks', label: 'reads + writes', kind: 'message' },
  { from: 'runs', to: 'jobs', label: 'claims', kind: 'message' },
  { from: 'jobs', to: 'runs', label: 'notifies worker', kind: 'message' },
  { from: 'chat', to: 'automations', label: 'hooks posts', kind: 'message' },
  { from: 'automations', to: 'tasks', label: 'creates', kind: 'message' },
  { from: 'automations', to: 'inbox', label: 'delivers', kind: 'message' },
  { from: 'forge', to: 'chat', label: 'posts tracker', kind: 'message' },
]

const legend = [
  { color: '#e6a94a', label: 'Product module' },
  { color: '#4fd0b6', label: 'System plane' },
  { color: '#8b93f0', label: 'Follow-up Msg / read' },
]

export function ModuleMap({ height = 560, direction = 'TB' }: { height?: number; direction?: 'TB' | 'LR' }) {
  return (
    <GraphFlow
      title="Product module interaction map"
      description="Edges are the only legal crossings: a host-drained follow-up message or a host-routed read through a types-only interface crate. pages, files, profiles, and vaults are fully self-contained; the agent collaboration loop clusters around runs, dispatch, and tagging."
      nodes={nodes}
      edges={edges}
      legend={legend}
      animatedKinds={['message']}
      height={height}
      direction={direction}
      nodesep={30}
      ranksep={64}
    />
  )
}
