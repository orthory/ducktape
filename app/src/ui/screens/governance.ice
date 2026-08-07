// APPROVALS — every decision this network is being asked to make, and the ones
// it has already settled. See `screens/roster.ice` for the screen contract.

component GovernanceScreen(rows:[ProposalRow], voting:str, admin:bool, answered:bool)
  emits
    gov_vote(str, bool)
    gov_execute(str)
  // THE SAME HEADER BAND AS THE OTHER TWO REGISTERS. This screen hand-rolled
  // its title inside the scroll, so it wore a different height (58px against
  // ScreenHeader's 56), carried no rule, no machine subtitle, and scrolled its
  // own title off the top while Members and Agents keep theirs pinned.
  // `proposals_summary` was written for exactly this seat and had never been
  // mounted anywhere.
  col w=fill h=fill
    ScreenHeader title="Approvals" meta=proposals_summary(rows)
      // The chip counts what is WAITING. Finalized rows have their own
      // section below and are never folded into this number.
      row gap=0.0
        if open_proposals(rows) > 0
          CountChip label=pending_label(rows)
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
        // The artifact bands the screen when the reader cannot vote. Its words
        // are ADMIN/MAINTAINER; ours are the tiers this chain actually grants.
        if !admin
          GateNote
            with
              reason="Approval votes are cast by this network's validators, and this node does not hold validator standing."
              next="You can still read every proposal and follow its tally while it runs."
        // Empty means nothing OPEN, and that is TWO different facts. One plate
        // for both claimed a history that may never have happened: on a fresh
        // network the header reads `0 open · 0 settled` while the plate said
        // every decision was finalized — asserting decisions nobody ever made.
        // A workspace whose every decision settled still gets its own plate,
        // never a silent screen.
        if empty(rows) && answered
          EmptyPlate message="No proposals yet — a membership or configuration change opens the first one."
        if open_proposals(rows) <= 0 && !empty(rows) && answered
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
