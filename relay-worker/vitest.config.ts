import { readFileSync } from "node:fs";
import { cloudflareTest } from "@cloudflare/vitest-plugin";
import { defineConfig } from "vitest/config";

// Synthetic test certificate only. No production keys, credentials or network.
const certificate = readFileSync(new URL("./test/fixtures/cert.pem", import.meta.url), "utf8");
export default defineConfig({
  plugins: [cloudflareTest({
    wrangler: { configPath: "./wrangler.jsonc" },
    miniflare: {
      cf: false,
      outboundService(request) {
        if (request.url !== "https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com") {
          return new Response(null, { status: 503 });
        }
        return Response.json({ "fixture-key": certificate }, {
          headers: { "Cache-Control": "public, max-age=3600" },
        });
      },
    },
  })],
  test: { testTimeout: 15000, hookTimeout: 30000, fileParallelism: false },
});
