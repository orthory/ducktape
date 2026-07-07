// Dev-only guest binding for the vendored tauri-agent-plugin. The runtime
// import is resolved by a Vite alias to the submodule source (see
// vite.config.ts); this ambient declaration is all tsc needs so the app bundle
// never compiles the submodule's TypeScript under our rootDir. Keep in sync
// with the constructor we actually use.
declare module "@byeongsu-hong/tauri-plugin-agent" {
  export class WebviewAgentInstrumentation {
    constructor(options: {
      windowLabel: string;
      state?: Record<string, () => unknown>;
    });
    install(): void;
  }
}
