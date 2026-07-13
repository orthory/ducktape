// The notice that stands in for the composer on an archived channel. The module
// rejects posts, reactions and huddle joins on an archived channel (edits and
// deletes still work), so the composer must not be left enabled to type into a
// write that will be silently refused — this says why instead, and offers the
// one op that reopens the channel to whoever may administer it (the same rule
// ChannelMenu's Unarchive obeys: owner-only when the channel has an owner).

import type { Channel } from "../../../domain/chat-client";
import { selfAuthorBytes } from "../../store/state";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { canAdministerChannel, selfAuthorKeyOf } from "./chat-helpers";
import { HoverButton } from "./HoverButton";

export function ArchivedNotice({ channel }: { channel: Channel }) {
  const { state, actions } = useDucktape();
  const selfKey = selfAuthorKeyOf(selfAuthorBytes(state.status, state.author));
  const canAdminister = canAdministerChannel(channel, selfKey);

  return (
    // Same outer padding/border as the Composer's frame — the notice occupies
    // exactly the slot the composer would have.
    <div
      style={{
        padding: "12px 16px 14px",
        borderTop: `1px solid ${color.borderSoft}`,
        background: color.paper,
        flexShrink: 0,
        minWidth: 0,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 10,
          padding: "11px 12px",
          borderRadius: radius.lg,
          border: `1px solid ${color.border}`,
          background: color.sunken,
          minWidth: 0,
        }}
      >
        <span style={{ flex: 1, minWidth: 0, font: `400 12.5px ${font.sans}`, color: color.muted3 }}>
          This channel is archived — new messages, reactions and huddles are closed.
        </span>
        {canAdminister && (
          <HoverButton
            onClick={() => actions.setChannelArchived(channel.id, false)}
            title="Reopen this channel"
            style={{
              flexShrink: 0,
              padding: "5px 10px",
              borderRadius: radius.sm,
              background: color.dark,
              color: color.onDark,
              font: `600 12px ${font.sans}`,
            }}
            hoverStyle={{ filter: "brightness(1.12)" }}
          >
            Unarchive
          </HoverButton>
        )}
      </div>
    </div>
  );
}
