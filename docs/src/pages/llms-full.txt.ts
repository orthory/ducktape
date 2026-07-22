// Full-corpus markdown for AI agents — every published page in one
// document. Scope and collation live in the framework helper; reshape or
// delete this route to change the site's corpus policy.
import { renderCorpusMarkdown } from "@cloudflare/nimbus-docs";

export const prerender = true;

export async function GET() {
  const markdown = (await renderCorpusMarkdown()).replace(
    /^import \{ [\w, ]+ \} from ["'][^"']+\/components\/diagram\/[^"']+["'];?$(?:\n)?/gm,
    "",
  );
  return new Response(markdown, {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
}
