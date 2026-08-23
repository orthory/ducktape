// APPROVALS — every decision this network is being asked to make, and the ones
// it has already settled. See `screens/roster.ice` for the screen contract.

component GovernanceScreen(rows:[ProposalRow], voting:str, admin:bool, connected:bool, answered:bool)
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
    ScreenHeader title="Approvals" meta=proposals_summary(connected, rows)
      // The chip counts what is WAITING. Finalized rows have their own
      // section below and are never folded into this number.
      row gap=0.0
        if connected && open_proposals(rows) > 0
          CountChip label=pending_label(rows)
    scroll #approvals-body
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
        if connected && !admin
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
        //
        // AND NOT CONNECTED IS NEITHER. Both plates below read the register;
        // with the node down there is no register, so they claimed a network
        // with no decisions off nothing. The GateNote goes with them — "this
        // node does not hold validator standing" is read off `members_rows`,
        // which is equally unreadable. Header and its chip stay.
        if !connected
          EmptyState
            with
              title="Not connected"
              description="Click the network name in the titlebar to pick or reconnect a network."
        if connected && empty(rows) && answered
          EmptyPlate
            with
              message="No proposals yet — a membership or configuration change opens the first one."
        if connected && open_proposals(rows) <= 0 && !empty(rows) && answered
          EmptyPlate message="No proposals waiting — every decision on this network is finalized."
        if connected && open_proposals(rows) > 0
          col w=fill gap=12.0
            for proposal in rows
              if proposal.open
                ProposalCard proposal=proposal busy=(!empty(voting))
                  forward
                    gov_vote
                    gov_execute
        // The FINALIZED eyebrow is gated on the settled subset, never on the
        // combined register — otherwise it hangs over nothing.
        if connected && !empty(settled_proposals(rows))
          col w=fill gap=10.0
            GroupLabel label="RECENTLY FINALIZED"
            for proposal in settled_proposals(rows)
              SettledProposalRow proposal=proposal
