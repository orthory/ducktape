import { useContext } from "react";

import { ConsoleContext } from "./DucktapeProvider";
import type { ConsoleContextValue } from "./DucktapeProvider";

export function useDucktape(): ConsoleContextValue {
  const value = useContext(ConsoleContext);
  if (!value) throw new Error("useDucktape requires a DucktapeProvider");
  return value;
}
