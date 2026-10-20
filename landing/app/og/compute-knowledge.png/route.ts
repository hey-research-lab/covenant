import { readFile } from "node:fs/promises";
import { join } from "node:path";

// Keep the page-specific card in the shared /og/<page>.png metadata contract.
// This artwork is pre-rendered with the official typefaces and original mark.
export const runtime = "nodejs";
export const dynamic = "force-static";

export async function GET() {
  const image = await readFile(join(process.cwd(), "public/compute-knowledge/share-card.png"));
  return new Response(new Uint8Array(image), {
    headers: {
      "content-type": "image/png",
      "cache-control": "public, max-age=3600, s-maxage=86400, stale-while-revalidate=604800",
    },
  });
}
