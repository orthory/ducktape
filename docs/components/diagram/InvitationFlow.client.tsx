'use client'

import { GraphFlow, type SemanticEdge, type SemanticNode } from './GraphFlow.client'

/**
 * Validator onboarding as a horizontal pipeline. The load-bearing split:
 * redeeming the invite grants *resident* standing (mesh + state-sync, no vote);
 * a separate promotion ceremony writes the key into the valset and only then
 * does the joiner boot as a full validator. Grounded in the invite/onboarding
 * records + valset admission.
 */

const nodes: SemanticNode[] = [
  { id: 'invite', label: 'Signed invite', sub: 'compact blob', category: 'step' },
  { id: 'contact', label: 'First contact', sub: 'direct or rendezvous', category: 'network' },
  { id: 'lobby', label: 'Lobby handshake', sub: 'derived transport key', category: 'network' },
  { id: 'resident', label: 'Redeem → resident', sub: 'mesh + state-sync, no vote', category: 'step' },
  { id: 'promote', label: 'Promote → valset', sub: 'member vote · quorum seat', category: 'system' },
  { id: 'validator', label: 'Boot validator', sub: 'sync + checkpoint', category: 'actor' },
]

const edges: SemanticEdge[] = [
  { from: 'invite', to: 'contact', label: 'dial' },
  { from: 'contact', to: 'lobby', label: 'handshake' },
  { from: 'lobby', to: 'resident', label: 'redeem', kind: 'admit' },
  { from: 'resident', to: 'promote', label: 'warm', kind: 'admit' },
  { from: 'promote', to: 'validator', label: 'admit', kind: 'admit' },
]

const legend = [
  { color: '#e6a94a', label: 'Onboarding step' },
  { color: '#9a9df0', label: 'First-contact transport' },
  { color: '#4fd0b6', label: 'Valset admission' },
  { color: '#67ade6', label: 'Live validator' },
]

export function InvitationFlow({ height = 320, direction = 'LR' }: { height?: number; direction?: 'TB' | 'LR' }) {
  return (
    <GraphFlow
      title="Invite → resident → validator"
      description="Redeeming the invite grants resident standing — mesh and state-sync access — but not a vote. A separate promotion writes the key into the valset; only then does the joiner sync the promotion boundary, checkpoint, and boot as a full validator."
      nodes={nodes}
      edges={edges}
      legend={legend}
      animatedKinds={['admit']}
      height={height}
      direction={direction}
      ranksep={72}
      nodesep={40}
    />
  )
}
