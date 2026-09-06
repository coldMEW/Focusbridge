import { expect, it } from "vitest";
import { FirebaseVerifier } from "../src/firebase";
import cert from "./fixtures/cert.pem?raw";
import { token } from "./helpers";

it("serves the synthetic public Google certificate through the runtime outbound boundary", async () => {
  const response = await fetch("https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com", { redirect: "manual", signal: AbortSignal.timeout(5000) });
  expect(response.status).toBe(200);
  expect(response.headers.get("Cache-Control")).toBe("public, max-age=3600");
  expect(await response.json()).toEqual({ "fixture-key": cert });
  expect(await new FirebaseVerifier().verify(await token({ sub: "synthetic-owner" }))).toBe("synthetic-owner");
});

it("caches Google certificates for max-age minus Age and coalesces cold requests", async () => {
  let now = Date.now();
  let calls = 0;
  const verifier = new FirebaseVerifier(async (input, init) => {
    calls++;
    expect(String(input)).toBe("https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com");
    expect(init?.headers).toBeUndefined();
    expect(init?.redirect).toBe("manual");
    return Response.json({ "fixture-key": cert }, { headers: { "Cache-Control": "public, max-age=100", Age: "20" } });
  }, () => now);
  const jwt = await token({ sub: "synthetic-owner" });
  expect(await Promise.all(Array.from({ length: 5 }, () => verifier.verify(jwt)))).toEqual(Array(5).fill("synthetic-owner"));
  expect(calls).toBe(1);
  now += 79000;
  expect(await verifier.verify(jwt)).toBe("synthetic-owner");
  expect(calls).toBe(1);
  now += 1001;
  await verifier.verify(jwt);
  expect(calls).toBe(2);
});
it("unknown kid cannot cause repeated fetches within provider TTL", async () => {
  let calls = 0;
  const verifier = new FirebaseVerifier(async () => {
    calls++;
    return Response.json({ "fixture-key": cert }, { headers: { "Cache-Control": "max-age=3600" } });
  });
  await verifier.verify(await token());
  for (let n = 0; n < 4; n++) await expect(verifier.verify(await token({}, { kid: `missing-${n}` }))).rejects.toThrow();
  expect(calls).toBe(1);
});
it("does not reuse stale keys during Google outage and bounds failure retries", async () => {
  let now = Date.now();
  let calls = 0;
  const verifier = new FirebaseVerifier(async () => {
    calls++;
    if (calls > 1) return new Response(null, { status: 503 });
    return Response.json({ "fixture-key": cert }, { headers: { "Cache-Control": "max-age=1" } });
  }, () => now);
  const jwt = await token();
  await verifier.verify(jwt);
  now += 1001;
  await expect(verifier.verify(jwt)).rejects.toThrow("identity_unavailable");
  await expect(verifier.verify(jwt)).rejects.toThrow("identity_unavailable");
  expect(calls).toBe(2);
});
it.each(["no-store", "public", "max-age=bogus"])("fails closed for unusable provider caching policy %s", async cacheControl => {
  const verifier = new FirebaseVerifier(async () => Response.json({ "fixture-key": cert }, { headers: { "Cache-Control": cacheControl } }));
  await expect(verifier.verify(await token())).rejects.toThrow("identity_unavailable");
});

it("rejects provider redirects without following a token-controlled or external key source", async () => {
  const verifier = new FirebaseVerifier(async (_input, init) => {
    expect(init?.redirect).toBe("manual");
    return new Response(null, { status: 302, headers: { Location: "https://attacker.invalid/certs" } });
  });
  await expect(verifier.verify(await token())).rejects.toThrow("identity_unavailable");
});

it("enforces bounded certificate documents and key counts", async () => {
  for (const body of [" ".repeat(65537), JSON.stringify(Object.fromEntries(Array.from({ length: 17 }, (_, i) => [String(i), cert])))]) {
    const verifier = new FirebaseVerifier(async () => new Response(body, { headers: { "Cache-Control": "max-age=60" } }));
    await expect(verifier.verify(await token())).rejects.toThrow("identity_unavailable");
  }
});
