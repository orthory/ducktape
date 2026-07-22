import { defineCollection } from "astro:content";
import { z } from "astro/zod";
import { docsCollection } from "@cloudflare/nimbus-docs/content";

export const collections = {
  docs: defineCollection(
    docsCollection({
      schemaFields: {
        audience: z.literal("human").optional(),
      },
    }),
  ),
};
