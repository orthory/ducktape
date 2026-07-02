// Console root: provider (owns the node transport) → window frame → shell.

import { DucktapeProvider } from "./store/DucktapeProvider";
import { ConsoleShell } from "./layout/ConsoleShell";
import { WindowFrame } from "./layout/WindowFrame";

export function DucktapeConsole() {
  return (
    <DucktapeProvider>
      <WindowFrame>
        <ConsoleShell />
      </WindowFrame>
    </DucktapeProvider>
  );
}
