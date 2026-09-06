import { describe, expect, it, vi } from "vitest";
import { evictDurableObject, runInDurableObject } from "cloudflare:test";
import { api, ownerRequest, provision, stub, token } from "./helpers";

describe("public Worker and verified owner provisioning", () => {
  it("has health, no permissive CORS or cacheable secret responses", async () => {
    const response = await api("/health");
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ status: "ok", protocol: 1 });
    expect(response.headers.get("Cache-Control")).toBe("no-store");
    expect(response.headers.has("Access-Control-Allow-Origin")).toBe(false);
  });
  it("rejects anonymous creation, listing and revocation", async () => {
    for (const [path, method] of [["/v1/pairs", "POST"], ["/v1/pairs", "GET"], ["/v1/pairs/" + "a".repeat(32), "DELETE"]]) {
      expect((await api(path!, { method })).status).toBe(401);
    }
  });
  it.each([
    { email_verified: false }, { email_verified: "true" }, { email_verified: null },
    { email: "" }, { aud: "attacker" }, { aud: ["foucsbridge"] },
    { iss: "https://attacker.invalid" }, { sub: "" }, { sub: "x".repeat(129) },
    { exp: 1 }, { exp: "9999999999" }, { iat: 9999999999 }, { iat: null },
    { auth_time: 9999999999 }, { auth_time: null }, { nbf: 9999999999 },
    { firebase: { tenant: "different-tenant" } },
    { firebase: { tenant: null } }, { firebase: null }, { firebase: [] },
  ])("rejects invalid signed claims %j", async claims => {
    const response = await ownerRequest("/v1/pairs", "POST", await token(claims));
    expect(response.status).toBe(401);
    expect(await response.json()).toEqual({ error: "unauthorized" });
  });
  it("rejects unknown signing key, bad signature, alg none and oversize bearer", async () => {
    const jwt = await token();
    const chunks = jwt.split(".");
    for (const value of [
      await token({}, { kid: "unknown" }),
      chunks[0] + "." + chunks[1] + "." + "A".repeat(342),
      "eyJhbGciOiJub25lIn0.e30.", "a".repeat(8193),
    ]) expect((await ownerRequest("/v1/pairs", "POST", value)).status).toBe(401);
  });
  it("provisions independent random role caps, persists hashes only and lists metadata only", async () => {
    const pair = await provision();
    expect(pair.accountKey).toMatch(/^[a-f0-9]{64}$/);
    expect(pair.pairId).toMatch(/^[a-f0-9]{32}$/);
    expect(pair.capabilities.desktop).toMatch(/^[A-Za-z0-9_-]{43}$/);
    expect(pair.capabilities.phone).not.toBe(pair.capabilities.desktop);
    expect(pair.expiresAt - pair.createdAt).toBe(30 * 86400000);
    const list = await ownerRequest("/v1/pairs");
    expect(await list.json()).toEqual({ accountKey: pair.accountKey, pairs: [{
      pairId: pair.pairId, createdAt: pair.createdAt, expiresAt: pair.expiresAt,
    }] });
    const stored = await runInDurableObject(stub(pair), (_obj, state) =>
      state.storage.sql.exec("SELECT * FROM pairs").toArray());
    expect(stored).toHaveLength(1);
    const serialized = JSON.stringify(stored);
    expect(serialized).not.toContain(pair.capabilities.desktop);
    expect(serialized).not.toContain(pair.capabilities.phone);
    expect(serialized).not.toContain("synthetic-owner");
    expect(serialized).not.toContain("@example.invalid");
    expect(stored[0]).toMatchObject({ desktop_hash: expect.stringMatching(/^[a-f0-9]{64}$/), phone_hash: expect.stringMatching(/^[a-f0-9]{64}$/) });
  });
  it("enforces max ten atomically under concurrent provision requests", async () => {
    const jwt = await token();
    const responses = await Promise.all(Array.from({ length: 14 }, () => ownerRequest("/v1/pairs", "POST", jwt)));
    expect(responses.filter(r => r.status === 201)).toHaveLength(10);
    expect(responses.filter(r => r.status === 409 || r.status === 429)).toHaveLength(4);
  });
  it("isolates owners and revocation survives real eviction", async () => {
    const pair = await provision();
    const other = await token({ sub: "other-owner" });
    expect((await (await ownerRequest("/v1/pairs", "GET", other)).json<{ pairs: unknown[] }>()).pairs).toEqual([]);
    expect((await ownerRequest(`/v1/pairs/${pair.pairId}`, "DELETE", other)).status).toBe(404);
    await evictDurableObject(stub(pair));
    expect((await ownerRequest(`/v1/pairs/${pair.pairId}`, "DELETE")).status).toBe(204);
    await evictDurableObject(stub(pair));
    expect((await (await ownerRequest("/v1/pairs")).json<{ pairs: unknown[] }>()).pairs).toEqual([]);
  });
  it("limits pair churn even after deletion and eviction", async () => {
    let pair = await provision();
    for (let n = 1; n < 20; n++) {
      expect((await ownerRequest(`/v1/pairs/${pair.pairId}`, "DELETE")).status).toBe(204);
      pair = await provision();
    }
    expect((await ownerRequest(`/v1/pairs/${pair.pairId}`, "DELETE")).status).toBe(204);
    await evictDurableObject(stub(pair));
    const response = await ownerRequest("/v1/pairs", "POST");
    expect(response.status).toBe(429);
    expect(await response.json()).toEqual({ error: "provision_limit" });
    const budget = await runInDurableObject(stub(pair), (_obj, state) =>
      state.storage.sql.exec<{ start: number }>("SELECT start FROM budgets WHERE name = 'provision'").one());
    const clock = vi.spyOn(Date, "now").mockReturnValue(budget.start - 1000);
    expect((await ownerRequest("/v1/pairs", "POST")).status).toBe(429);
    clock.mockReturnValue(budget.start + 3600000);
    // Exercise the storage clock independently of JWT validation's real Date.
    expect(await runInDurableObject(stub(pair), async obj =>
      (await obj.manage(pair.accountKey, "POST")).status)).toBe(201);
  });
  it("does not accept app secrets/metadata or credentials in URLs", async () => {
    expect((await ownerRequest("/v1/pairs", "POST", undefined, '{"enrollmentPsk":"never-send-this"}')).status).toBe(400);
    expect((await ownerRequest("/v1/pairs?token=never-send-this", "POST")).status).toBe(400);
    expect((await ownerRequest("/v1/pairs", "POST", undefined, " ".repeat(1025))).status).toBe(413);
    expect((await api("http://ignored")).status).toBe(404);
  });
});
