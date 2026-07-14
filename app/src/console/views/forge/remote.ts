// The remote origin url the forge surface reads through — a direct remote
// client's node http base — or null when browsing the local workspace repo.
// ForgeView provides it once; the tab/detail components (commit diffs, PR
// compare, merge build) pick it up for their own git reads instead of
// threading it through every prop chain.

import { createContext, useContext } from "react";

export const ForgeRemoteContext = createContext<string | null>(null);

export const useForgeRemote = (): string | null => useContext(ForgeRemoteContext);
