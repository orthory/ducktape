// Read a file from the repo, for tests that pin a TS mirror against the Rust
// source it mirrors (a consensus rule the client must not drift from). Vite
// serves modules over http, so `import.meta.url` is not a file path — walk up
// from the test runner's cwd to the workspace root instead.

import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

export const repoFile = (relative: string): string => {
  let dir = process.cwd();
  while (!existsSync(resolve(dir, "Cargo.toml"))) {
    const up = dirname(dir);
    if (up === dir) throw new Error(`no repo root above ${process.cwd()}`);
    dir = up;
  }
  return readFileSync(resolve(dir, relative), "utf8");
};
