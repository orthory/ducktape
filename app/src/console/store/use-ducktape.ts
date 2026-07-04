import { useContext } from "react";

import { ConsoleContext, type ConsoleContextValue } from "./context";

export function useDucktape(): ConsoleContextValue {
  const value = useContext(ConsoleContext);
  if (!value) throw new Error("useDucktape requires a DucktapeProvider");
  return value;
}
