import "@testing-library/jest-dom/vitest";

import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// vitest globals are off, so react-testing-library's automatic unmount hook
// never registers itself — do it explicitly or renders leak across tests
afterEach(cleanup);
