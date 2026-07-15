import "@testing-library/jest-dom/vitest";

import { cleanup, configure } from "@testing-library/react";
import { afterEach, vi } from "vitest";

// The jsdom UI suite runs many files in parallel forks; under CPU
// oversubscription a `waitFor`/`findBy` occasionally exceeds its 1000ms default
// and flakes (observed >10s under a 3-way build+test contended box). Give the
// async utils real headroom, kept below the 15s testTimeout in vite.config.ts
// so a genuine hang still surfaces as a clean waitFor failure, not a bare
// test-timeout.
configure({ asyncUtilTimeout: 10_000 });

type NativeInvoke = (feature: string, args?: unknown) => unknown;
type NativeEvents = {
  emit(event: string, payload?: unknown): Promise<void>;
  listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<() => void>;
};

type TestGlobals = typeof globalThis & {
  __DUCKTAPE_TEST_NATIVE_INVOKE__?: NativeInvoke;
  __DUCKTAPE_TEST_NATIVE_EVENTS__?: NativeEvents;
};

const testGlobals = (): TestGlobals => globalThis as TestGlobals;

// Production intentionally has no web-to-desktop bridge. Tests that exercise
// shared console behavior can install a scoped adapter without adding one to
// the shipped bundle.
vi.mock("./domain/node-bootstrap", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./domain/node-bootstrap")>();
  return {
    ...actual,
    hasNativeShell: () => typeof testGlobals().__DUCKTAPE_TEST_NATIVE_INVOKE__ === "function",
    nativeCall: <T>(feature: string, args?: unknown): Promise<T> => {
      if (feature === "native window events" && testGlobals().__DUCKTAPE_TEST_NATIVE_EVENTS__) {
        return Promise.resolve(testGlobals().__DUCKTAPE_TEST_NATIVE_EVENTS__ as T);
      }
      const invoke = testGlobals().__DUCKTAPE_TEST_NATIVE_INVOKE__;
      return invoke
        ? Promise.resolve((args === undefined ? invoke(feature) : invoke(feature, args)) as T)
        : Promise.reject(new Error(`${feature} is available only in the native desktop app`));
    },
  };
});

// vitest globals are off, so react-testing-library's automatic unmount hook
// never registers itself — do it explicitly or renders leak across tests
afterEach(() => {
  cleanup();
  delete testGlobals().__DUCKTAPE_TEST_NATIVE_INVOKE__;
  delete testGlobals().__DUCKTAPE_TEST_NATIVE_EVENTS__;
});
