// The duck:// open plane: one typed ref → the store's EXISTING navigation
// vocabulary (the same targets the desktop-notification NavigateTarget
// patches). The protocol adds no navigation machinery — only a textual
// address for what the console can already do. Module table + classify live
// in domain/duck-uri.ts; chips call this on click, and the browser address
// bar hands module-plane URIs here.

import type { DuckRef } from "../../domain/duck-uri";
import { forgeItemTarget } from "../../domain/forge-client";
import type { ConsoleActions } from "./actions";

export const openDuckRef = (ref: DuckRef, actions: ConsoleActions): void => {
  if ("page" in ref) {
    // openPage loads the tree but does NOT navigate — the pages screen has
    // to be entered too (SearchModal pairs them the same way).
    actions.openPage(ref.page.id);
    actions.setScreen("pages");
  } else if ("file" in ref) {
    actions.openFiles(ref.file.path);
  } else if ("forge" in ref) {
    const { repo, number, seq } = ref.forge;
    if (number === null) actions.openForgeItem({ repo, number: null });
    else actions.openForgeItem({ repo, number, ...(seq ? { messageSeq: seq } : {}) });
  } else {
    const { id, seq } = ref.channel;
    // A forge item's hidden discussion channel (`forge:<repo>:<n>`) is
    // unroutable on the chat surface — route to the item view instead (the
    // navigate listener's rule).
    const forge = forgeItemTarget(id, { messageSeq: seq });
    if (forge) {
      actions.openForgeItem(forge);
    } else if (seq !== undefined) {
      actions.focusMessage(id, seq); // lands on the chat screen itself
    } else {
      actions.setScreen("chat");
      actions.selectChannel(id);
    }
  }
};
