// APPROVALS — every decision this network is being asked to make, and the ones
// it has already settled. See `screens/roster.ice` for the screen contract.

component GovernanceScreen(rows:[ProposalRow], voting:str, admin:bool, answered:bool)
  emits
    gov_vote(str, bool)
    gov_execute(str)
  scroll
    with
      dir=vertical
      w=fill
      h=fill
    col
      with
        w=fill
        p=22.0
        gap=16.0
      row gap=9.0 align=center
        text "Approvals"
          with
            size=16.0
            wrap=none
            font=display
            @text-primary
        // The chip counts what is WAITING. Finalized rows have their own
        // section below and are never folded into this number.
        if open_proposals(rows) > 0
          CountChip label=pending_label(rows)
      // The artifact bands the screen when the reader cannot vote. Its words
      // are ADMIN/MAINTAINER; ours are the tiers this chain actually grants.
      if !admin
        GateNote
          with
            reason="Approval votes are cast by this network's validators, and this node does not hold validator standing."
            next="You can still read every proposal and follow its tally while it runs."
      // Empty means nothing OPEN. A workspace whose every decision settled
      // still gets the plate, not a silent screen.
      if open_proposals(rows) <= 0 && answered
        EmptyPlate message="No proposals waiting — every decision on this network is finalized."
      if open_proposals(rows) > 0
        col w=fill gap=12.0
          for proposal in rows
            if proposal.open
              ProposalCard proposal=proposal busy=(!empty(voting))
                forward
                  gov_vote
                  gov_execute
      // The FINALIZED eyebrow is gated on the settled subset, never on the
      // combined register — otherwise it hangs over nothing.
      if !empty(settled_proposals(rows))
        col w=fill gap=10.0
          GroupLabel label="RECENTLY FINALIZED"
          for proposal in settled_proposals(rows)
            SettledProposalRow proposal=proposal
