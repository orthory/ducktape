// APPROVALS, as a module-owned view. The screen is a pure function of the
// props the host pushes (`props()` — one item per change) and speaks back in
// two intents. It shares the desktop app's theme file, so its ink and plates
// are the app's own tokens; what it cannot share yet is the app's shared kit
// (named fonts, `wrap=none`, line heights and component uses do not cross
// the tree wire), so the plates below are the kit's shapes spelled flat, in
// the wire's vocabulary.
app GovernanceView
  title "Approvals"
  palette active_palette
  id "dev.ducktape.view.governance"
  text-size 13.5

use "../../../../../app/src/ui/theme.ice"

extern crate::host
  HostError(message:str)
  ProposalRow(id:str, action:str, detail:str, proposer:str, status:str, deadline:i64, approvals:i64, rejections:i64, rule:str, required_yes:i64, electorate:i64, open:bool, settled_height:i64)
  GovernanceProps(rows:[ProposalRow], voting:str, admin:bool, connected:bool, answered:bool, dark:bool)
  QuorumSeat(filled:bool)
  stream props() -> GovernanceProps ! HostError
  sync vote(proposal_id:str, approve:bool) -> bool
  sync execute(proposal_id:str) -> bool
  pure proposals_summary(connected:bool, rows:&[ProposalRow]) -> str
  pure pending_label(rows:&[ProposalRow]) -> str
  pure open_proposals(rows:&[ProposalRow]) -> i64
  pure settled_proposals(rows:&[ProposalRow]) -> [ProposalRow]
  pure quorum_dots(approvals:i64, required:i64) -> [QuorumSeat]
  pure tally_label(approvals:i64, required:i64) -> str
  pure tally_tone(approvals:i64, required:i64) -> str
  pure tally_note(approvals:i64, required:i64) -> str
  pure approve_label(approvals:i64, required:i64) -> str
  pure proposal_kind_tone(action:&str) -> str
  pure height_label_short(height:i64) -> str

state
  active_palette:palette[AppTheme] = AppTheme.app
  rows:[ProposalRow] = []
  voting = ""
  admin = false
  connected = false
  answered = false
  host_error = ""

// The register is the host's: one subscription, one item per change.
on mount
  stream every props() -> props_changed _ | props_failed _

on props_changed(next)
  rows = next.rows
  voting = next.voting
  admin = next.admin
  connected = next.connected
  answered = next.answered
  active_palette = AppTheme.app
  return if !next.dark
  active_palette = AppTheme.app_dark

on props_failed(error)
  host_error = error.message

on gov_vote(proposal_id, approve)
  return if !connected || !empty(voting)
  let _sent = vote(proposal_id, approve)

on gov_execute(proposal_id)
  return if !connected || !empty(voting)
  let _sent = execute(proposal_id)

// The card's pieces as components — the tree target inlines a component
// use now instead of lowering it to native layout memoization.

component ProposalKindPill(action:str)
  col #root
    if proposal_kind_tone(action) == "access"
      box
        with
          px=6.0
          py=2.0
          bg=brand_bg
          r=4.0
        text action
          with
            size=9.0
            @text-brand
            @font-mono
            @font-semibold
    if proposal_kind_tone(action) != "access"
      box
        with
          px=6.0
          py=2.0
          bg=elevated
          r=4.0
        text action
          with
            size=9.0
            @text-avatar_fg_sm
            @font-mono
            @font-semibold

component QuorumDot(filled:bool)
  col #root
    if filled
      box
        with
          w=13.0
          h=13.0
          bg=success_dot
          r=6.5
        space w=1.0 h=1.0
    if !filled
      box
        with
          w=13.0
          h=13.0
          bg=surface
          border=presence_off
          border-w=1.5
          r=6.5
        space w=1.0 h=1.0

component SettledProposalRow(proposal:ProposalRow)
  box #root
    with
      w=fill
      px=15.0
      py=13.0
      bg=surface
      border=separator
      border-w=1.0
      r=10.0
    row
      with
        w=fill
        gap=11.0
        align=center
      box
        with
          w=19.0
          h=19.0
          align-x=center
          align-y=center
          bg=success_bg
          border=success_line
          border-w=1.0
          r=9.5
        text "✓"
          with
            size=9.0
            @text-success
            @font-mono
            @font-semibold
      text proposal.id
        with
          size=13.0
          @text-muted
          @font-medium
      space w=fill
      row gap=5.0 align=center
        text proposal.status
          with
            size=11.0
            @text-meta
            @font-mono
            @font-medium
        text "·"
          with
            size=11.0
            @text-meta
            @font-mono
            @font-medium
        text tally_label(proposal.approvals, proposal.required_yes)
          with
            size=11.0
            @text-meta
            @font-mono
            @font-medium
        // NO ✓ WITHOUT A HEIGHT BEHIND IT: a row whose op
        // predates the settle fold has 0 and prints nothing
        // rather than `h 0`.
        if proposal.settled_height > 0
          text "·"
            with
              size=11.0
              @text-meta
              @font-mono
              @font-medium
        if proposal.settled_height > 0
          text height_label_short(proposal.settled_height)
            with
              size=11.0
              @text-meta
              @font-mono
              @font-medium

view
  box #root
    with
      w=fill
      h=fill
      bg=bg
    col w=fill h=fill
      // THE SAME HEADER BAND AS THE OTHER REGISTERS: title, machine subtitle,
      // and a chip counting what is WAITING — finalized rows have their own
      // section and are never folded into this number.
      box
        with
          w=fill
          h=56.0
          px=22.0
        row
          with
            w=fill
            h=fill
            gap=10.0
            align=center
          text "Approvals" #title
            with
              size=16.0
              @text-primary
              @font-semibold
          text proposals_summary(connected, rows) #meta
            with
              size=12.0
              @text-hint
              @font-mono
          if connected && open_proposals(rows) > 0
            box #pending
              with
                px=8.0
                py=3.0
                bg=brand_bg
                r=6.0
              text pending_label(rows)
                with
                  size=11.0
                  @text-brand
                  @font-mono
                  @font-medium
          space w=fill
      box
        with
          w=fill
          h=1.0
          bg=separator
        space w=1.0 h=1.0
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
          if !empty(host_error)
            text host_error #host-error size=12.0 @text-danger
          // The reader cannot vote: say so in this chain's own words.
          if connected && !admin
            box #gate
              with
                w=fill
                px=13.0
                py=11.0
                bg=warning_bg_lit
                border=warning_line
                border-w=1.0
                r=9.0
              row
                with
                  w=fill
                  gap=8.0
                  align=start
                col pt=4.0
                  box
                    with
                      w=6.0
                      h=6.0
                      bg=warning_dot
                      r=3.0
                    space w=1.0 h=1.0
                col w=fill gap=2.0
                  text "Approval votes are cast by this network's validators, and this node does not hold validator standing."
                    with
                      w=fill
                      size=12.0
                      @text-warning_fg
                  text "You can still read every proposal and follow its tally while it runs."
                    with
                      w=fill
                      size=12.0
                      @text-warning
          // Not connected, nothing open, and nothing ever are THREE facts.
          if !connected
            box #offline
              with
                w=fill
                h=fill
                p=22.0
                align-x=center
                align-y=center
              col
                with
                  w=fill
                  align=center
                  gap=7.0
                box
                  with
                    w=42.0
                    h=42.0
                    align-x=center
                    align-y=center
                    bg=surface
                    border=border
                    border-w=1.0
                    r=21.0
                  text "◇" size=20.0 @text-primary
                text "Not connected"
                  with
                    size=16.0
                    @text-fg
                    @font-semibold
                text "Click the network name in the titlebar to pick or reconnect a network."
                  with
                    size=12.5
                    @text-caption
          // A dashed empty plate in the kit; a hairline one here — the wire's
          // border has no dash pattern.
          if connected && empty(rows) && answered
            box #empty
              with
                w=fill
                p=30.0
                align-x=center
                border=border
                border-w=1.0
                r=12.0
              text "No proposals yet — a membership or configuration change opens the first one."
                with
                  size=13.0
                  @text-meta
          if connected && open_proposals(rows) <= 0 && !empty(rows) && answered
            box #all-settled
              with
                w=fill
                p=30.0
                align-x=center
                border=border
                border-w=1.0
                r=12.0
              text "No proposals waiting — every decision on this network is finalized."
                with
                  size=13.0
                  @text-meta
          if connected && open_proposals(rows) > 0
            col #open w=fill gap=12.0
              for proposal in rows
                if proposal.open
                  box
                    with
                      w=fill
                      p=16.0
                      bg=surface
                      border=border
                      border-w=1.0
                      r=12.0
                    col w=fill gap=5.0
                      row
                        with
                          w=fill
                          gap=7.0
                          align=center
                        // ACCESS-class proposals wear the terracotta pair;
                        // everything else is neutral.
                        ProposalKindPill action=proposal.action
                        text proposal.id
                          with
                            w=fill
                            size=14.0
                            @text-primary
                            @font-semibold
                      // one meta line: who opened it, when it lapses, and
                      // what it actually does
                      row
                        with
                          w=fill
                          gap=4.0
                          align=center
                        text "proposed by" size=12.0 @text-caption
                        row gap=0.0 align=center
                          text "@" size=12.0 @text-secondary_fg
                          text proposal.proposer size=12.0 @text-secondary_fg
                        text "· expires at h" size=12.0 @text-caption
                        text proposal.deadline
                          with
                            size=12.0
                            @text-secondary_fg
                            @font-mono
                            @font-medium
                        if !empty(proposal.detail)
                          text "·" size=12.0 @text-caption
                        if !empty(proposal.detail)
                          text proposal.detail
                            with
                              w=fill
                              size=12.0
                              @text-secondary_fg
                              @font-mono
                              @font-medium
                      // the dots count the frozen voting rule, not the
                      // electorate; an unfilled seat is the unfinalized ring
                      row
                        with
                          w=fill
                          gap=13.0
                          align=center
                          pt=9.0
                        row gap=5.0 align=center
                          for seat in quorum_dots(proposal.approvals, proposal.required_yes)
                            QuorumDot filled=seat.filled
                        // `3 / 4` in one mono run: grey until one signature
                        // from quorum, then green
                        if tally_tone(proposal.approvals, proposal.required_yes) == "near"
                          text tally_label(proposal.approvals, proposal.required_yes)
                            with
                              size=12.0
                              @text-success
                              @font-mono
                              @font-semibold
                        if tally_tone(proposal.approvals, proposal.required_yes) != "near"
                          text tally_label(proposal.approvals, proposal.required_yes)
                            with
                              size=12.0
                              @text-meta
                              @font-mono
                              @font-semibold
                        text tally_note(proposal.approvals, proposal.required_yes)
                          with
                            size=12.0
                            @text-meta
                        if proposal.rejections > 0
                          text proposal.rejections
                            with
                              size=12.0
                              @text-danger
                              @font-mono
                              @font-medium
                        if proposal.rejections > 0
                          text "against" size=12.0 @text-caption
                        space w=fill
                        // exactly two buttons; Settle appears only once the
                        // rule is met, because that is the only moment it
                        // can succeed
                        row gap=8.0 align=center
                          button #reject -> gov_vote(proposal.id, false)
                            with
                              label="Reject"
                              disabled=(!empty(voting))
                              p=8.0
                            active bg=surface text=secondary_fg border=control_line border-w=1.0 r=8.0
                            hovered bg=row_hover text=secondary_fg border=control_line_hover border-w=1.0 r=8.0
                            disabled bg=surface text=disabled_fg border=control_line border-w=1.0 r=8.0
                            text "Reject" size=12.0 @text-secondary_fg
                          if proposal.approvals < proposal.required_yes
                            button #approve -> gov_vote(proposal.id, true)
                              with
                                label="Approve"
                                disabled=(!empty(voting))
                                p=8.0
                              active bg=primary text=primary_fg r=8.0
                              hovered bg=primary_hover text=primary_fg r=8.0
                              disabled bg=disabled text=disabled_fg r=8.0
                              text approve_label(proposal.approvals, proposal.required_yes)
                                with
                                  size=12.0
                                  @text-primary_fg
                          if proposal.approvals >= proposal.required_yes
                            button #settle -> gov_execute(proposal.id)
                              with
                                label="Settle"
                                disabled=(!empty(voting))
                                p=8.0
                              active bg=secondary text=secondary_fg r=8.0
                              hovered bg=elevated text=secondary_fg r=8.0
                              disabled bg=disabled text=disabled_fg r=8.0
                              text "Settle →" size=12.0 @text-secondary_fg
          // The FINALIZED eyebrow is gated on the settled subset, never on
          // the combined register — otherwise it hangs over nothing.
          if connected && !empty(settled_proposals(rows))
            col #settled w=fill gap=10.0
              text "RECENTLY FINALIZED"
                with
                  size=9.0
                  @text-label
                  @font-mono
                  @font-semibold
              // A settled proposal: a tick, the title, and the tally it
              // closed on.
              for proposal in settled_proposals(rows)
                SettledProposalRow proposal=proposal
