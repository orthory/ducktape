// Pure layout maths for the huddle video stage. React-free so the decisions are
// unit-tested directly; the stage component (HuddleStage.tsx) renders from these.

import type { HuddleParticipant } from "../../store/huddle-roster";

/** Gallery column count for `count` tiles — grows with √count (so tiles stay
 *  roughly square), clamped to [1, 4] to keep tiles legible in a real huddle. */
export const galleryColumns = (count: number): number =>
  Math.min(4, Math.max(1, Math.ceil(Math.sqrt(Math.max(1, count)))));

/** Which participant the spotlight shows: an explicit pin (if still present),
 *  else the active speaker (only the self row can report speaking today), else
 *  the first member. Null for an empty roster. */
export const spotlightKey = (
  participants: HuddleParticipant[],
  pinned: string | null,
): string | null => {
  if (pinned && participants.some((p) => p.key === pinned)) return pinned;
  const speaker = participants.find((p) => p.speaking);
  if (speaker) return speaker.key;
  return participants[0]?.key ?? null;
};
