import { defineConfig } from "astro/config";
import react from "@astrojs/react";
import icon from "astro-icon";
import tailwindcss from "@tailwindcss/vite";
import nimbus, { defineConfig as defineNimbusConfig } from "@cloudflare/nimbus-docs";
import { tableScroll } from "@cloudflare/nimbus-docs/markdown";

const page = (prefix: string, path: string, label: string) => ({
  label,
  link: `/${prefix}${path ? `/${path}` : ""}`,
});

const humanTrack = (prefix: string, overview: string) => [
  page(prefix, "", overview),
  page(prefix, "start/quick-start", "Quick Start"),
  {
    label: "Architecture",
    items: [
      page(prefix, "architecture/platform-invariants", "Platform Invariants"),
      page(prefix, "architecture/module-model", "Module Model"),
      page(prefix, "architecture/consensus-and-node", "Consensus and Node"),
      page(prefix, "architecture/async-engine", "Async Engine"),
      page(prefix, "architecture/state-sync", "State Sync"),
    ],
  },
  page(prefix, "network/network-and-membership", "Network and Membership"),
  page(prefix, "network/coordination", "Coordination"),
  page(prefix, "modules/product-modules", "Product Modules"),
  page(prefix, "roadmap/what-is-left", "What Is Left"),
  {
    label: "Reference",
    items: [
      page(prefix, "reference/repository-map", "Repository Map"),
      page(prefix, "reference/implementation-status", "Implementation Status"),
      page(prefix, "reference/design-records", "Design Records"),
      page(prefix, "reference/gotchas", "Gotchas"),
    ],
  },
];

const agentTrack = (prefix: string, overview: string) => [
  page(prefix, "", overview),
  page(prefix, "start/operating-loop", "Operating Loop"),
  {
    label: "Contracts",
    items: [
      page(prefix, "architecture/determinism-contract", "Determinism Contract"),
      page(prefix, "architecture/state-sync-contract", "State Sync Contract"),
      page(prefix, "network/validator-operations", "Validator Operations"),
    ],
  },
  page(prefix, "roadmap/open-work", "Open Work"),
  {
    label: "Reference",
    items: [
      page(prefix, "reference/repository-map", "Repository Map"),
      page(prefix, "reference/verification-matrix", "Verification Matrix"),
      page(prefix, "reference/design-records", "Design Records"),
      page(prefix, "reference/gotchas", "Gotchas"),
    ],
  },
];

const nimbusConfig = defineNimbusConfig({
  site: process.env.DOCS_SITE_URL ?? "http://localhost:4321",
  title: "Ducktape",
  description:
    "A consensus-based workplace super-app built as one BFT-replicated state machine.",
  locale: "en",
  github: "https://github.com/orthory/ducktape",
  editPattern: "https://github.com/orthory/ducktape/edit/dev/docs/{path}",
  socialImageAlt: "Ducktape documentation preview",
  sidebar: {
    scope: "section",
    items: [
      {
        label: "Human · English",
        segment: "/en/human",
        items: humanTrack("en/human", "Overview"),
      },
      {
        label: "Human · 한국어",
        segment: "/ko/human",
        items: humanTrack("ko/human", "개요"),
      },
      {
        label: "Agent · English",
        segment: "/en/agent",
        items: agentTrack("en/agent", "Overview"),
      },
      {
        label: "Agent · 한국어",
        segment: "/ko/agent",
        items: agentTrack("ko/agent", "개요"),
      },
    ],
  },
});

export default defineConfig({
  output: "static",
  vite: { plugins: [tailwindcss()] },
  prefetch: { prefetchAll: true, defaultStrategy: "hover" },
  integrations: [
    react(),
    icon(),
    nimbus(nimbusConfig, {
      rules: {
        "nimbus/frontmatter-shape": "error",
        "nimbus/internal-link": "error",
      },
      markdown: { hastPlugins: [tableScroll()] },
    }),
  ],
});
