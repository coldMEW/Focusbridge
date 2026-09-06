import { expect, it, vi } from "vitest";
import { evictDurableObject, runDurableObjectAlarm, runInDurableObject } from "cloudflare:test";
import { api, connect, ownerRequest, provision, socketPath, stub, upgrade } from "./helpers";
import { MAX_SESSION_BYTES, MAX_SESSION_MESSAGES, SESSION_MS } from "../src/account";

it("does not force reconnect every ten minutes or after two inventory records", async () => {
  let now = Date.now();
  vi.spyOn(Date, "now").mockImplementation(() => now);
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  const phone = await connect(pair, "phone");
  now += 11 * 60 * 1000;
  await evictDurableObject(stub(pair));
  // Two encrypted 1-MiB records include framing/tag overhead, exceeding 2 MiB.
  for (let n = 0; n < 36; n++) phone.ws.send(new Uint8Array(61472));
  await expect.poll(() => desktop.binary.length).toBe(36);
  expect(phone.closeCode).toBeUndefined();
  expect(desktop.closeCode).toBeUndefined();
  expect(SESSION_MS).toBe(24 * 60 * 60 * 1000);
});

it("authenticates roles; routing metadata, wrong caps and owner-header spoofing cannot authorize", async () => {
  const pair = await provision();
  expect((await api(socketPath(pair, "desktop"), { headers: { Upgrade: "websocket" } })).status).toBe(401);
  expect((await upgrade(pair, "desktop", pair.capabilities.phone)).status).toBe(401);
  expect((await upgrade(pair, "phone", "A".repeat(43))).status).toBe(401);
  expect((await api(socketPath(pair, "admin"), { headers: { Upgrade: "websocket", Authorization: `Bearer ${pair.capabilities.phone}` } })).status).toBe(404);
  expect((await api(socketPath(pair, "phone"), { headers: { Authorization: `Bearer ${pair.capabilities.phone}` } })).status).toBe(426);
  const another = await provision();
  expect((await upgrade(another, "phone", pair.capabilities.phone)).status).toBe(401);
  expect((await api("/v1/pairs", { method: "POST", headers: { "X-Owner": pair.accountKey } })).status).toBe(401);
});

it("forwards opaque binary unchanged both ways with transport-only generation metadata", async () => {
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  const phone = await connect(pair, "phone");
  desktop.ws.send(new Uint8Array([0, 255, 0, 17]));
  phone.ws.send(new Uint8Array([99, 18]));
  await expect.poll(() => phone.binary.length).toBe(1);
  await expect.poll(() => desktop.binary.length).toBe(1);
  expect([...new Uint8Array(phone.binary[0]!)]).toEqual([0, 255, 0, 17]);
  expect([...new Uint8Array(desktop.binary[0]!)]).toEqual([99, 18]);
  const ready = desktop.controls.find(m => m.type === "relay.peer_ready");
  expect(ready).toEqual({ type: "relay.peer_ready", generation: expect.stringMatching(/^[a-f0-9]{32}$/) });
  expect(phone.controls).toContainEqual(ready);
  expect(JSON.stringify(desktop.controls)).not.toContain("authenticated");
});

it("reports unavailable and drops absent-peer bytes without any offline queue", async () => {
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  desktop.ws.send(new Uint8Array([42]));
  await expect.poll(() => desktop.closeCode).toBe(1012);
  expect(desktop.controls).toContainEqual({ type: "relay.peer_unavailable" });
  const phone = await connect(pair, "phone");
  const fresh = await connect(pair, "desktop");
  fresh.ws.send(new Uint8Array([43]));
  await expect.poll(() => phone.binary.length).toBe(1);
  expect([...new Uint8Array(phone.binary[0]!)]).toEqual([43]);
  const tables = await runInDurableObject(stub(pair), (_obj, state) => state.storage.sql.exec("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE '_cf_%' ORDER BY name").toArray());
  expect(tables.map(t => t.name)).toEqual(["budgets", "pairs"]);
});

it("restores both role owners and association fences through actual workerd hibernation", async () => {
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  const phone = await connect(pair, "phone");
  await evictDurableObject(stub(pair), { webSockets: "hibernate" });
  desktop.ws.send(new Uint8Array([1]));
  await expect.poll(() => phone.binary.length).toBe(1);
  await evictDurableObject(stub(pair));
  phone.ws.send(new Uint8Array([2]));
  await expect.poll(() => desktop.binary.length).toBe(1);
});

it.each(["desktop", "phone"] as const)("replacement of %s invalidates BOTH generations and stale close/message callbacks", async role => {
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  const phone = await connect(pair, "phone");
  const other = role === "desktop" ? "phone" : "desktop";
  const handle = stub(pair);
  let stale: WebSocket | undefined;
  await runInDurableObject(handle, (_obj, state) => { stale = state.getWebSockets(pair.pairId).find(ws => ws.deserializeAttachment().role === other); });
  const generation = desktop.controls.find(m => m.type === "relay.peer_ready")?.generation;
  const fresh = await connect(pair, role);
  await expect.poll(() => desktop.closeCode).toBe(1012);
  await expect.poll(() => phone.closeCode).toBe(1012);
  expect(desktop.controls).toContainEqual({ type: "relay.peer_unavailable" });
  const freshPeer = await connect(pair, other);
  // Reproduce a queued event with the actual obsolete server socket, not a mock.
  await runInDurableObject(handle, (obj) => {
    obj.webSocketMessage(stale!, new Uint8Array([0xde, 0xad]).buffer);
    obj.webSocketClose(stale!, 1000, "late_close", true);
  });
  await evictDurableObject(handle);
  freshPeer.ws.send(new Uint8Array([0xbe, 0xef]));
  await expect.poll(() => fresh.binary.length).toBe(1);
  expect([...new Uint8Array(fresh.binary[0]!)]).toEqual([0xbe, 0xef]);
  expect(freshPeer.binary).toHaveLength(0);
  expect(fresh.controls.find(m => m.type === "relay.peer_ready")?.generation).not.toBe(generation);
});

it("a departing role retires the old opposite socket and both must rejoin", async () => {
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  const phone = await connect(pair, "phone");
  phone.ws.close(1000, "leaving");
  await expect.poll(() => desktop.closeCode).toBe(1012);
  expect(desktop.controls).toContainEqual({ type: "relay.peer_unavailable" });
});

it.each(["desktop", "phone"] as const)("rechecks %s capability expiry on every frame after hibernation", async expiredRole => {
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  const phone = await connect(pair, "phone");
  await runInDurableObject(stub(pair), (_obj, state) => {
    state.storage.sql.exec(`UPDATE pairs SET ${expiredRole}_expires_at = ? WHERE pair_id = ?`, Date.now() - 1, pair.pairId);
  });
  await evictDurableObject(stub(pair));
  desktop.ws.send(new Uint8Array([12]));
  await expect.poll(() => desktop.closeCode).toBe(4003);
  expect(phone.binary).toHaveLength(0);
});

it.each(["desktop", "phone"] as const)("rechecks %s capability generation on every frame after hibernation", async revokedRole => {
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  const phone = await connect(pair, "phone");
  await runInDurableObject(stub(pair), (_obj, state) => {
    state.storage.sql.exec(`UPDATE pairs SET ${revokedRole}_cap_generation = ${revokedRole}_cap_generation + 1 WHERE pair_id = ?`, pair.pairId);
  });
  await evictDurableObject(stub(pair));
  desktop.ws.send(new Uint8Array([13]));
  await expect.poll(() => desktop.closeCode).toBe(4003);
  expect(phone.binary).toHaveLength(0);
});

it("owner revocation closes both sockets and old caps fail after eviction", async () => {
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  const phone = await connect(pair, "phone");
  expect((await ownerRequest(`/v1/pairs/${pair.pairId}`, "DELETE")).status).toBe(204);
  await expect.poll(() => desktop.closeCode).toBe(4003);
  await expect.poll(() => phone.closeCode).toBe(4003);
  await evictDurableObject(stub(pair));
  expect((await upgrade(pair, "desktop")).status).toBe(401);
});

it.each(["text", "empty", "oversize"])("rejects %s application messages before forwarding", async kind => {
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  const phone = await connect(pair, "phone");
  desktop.ws.send(kind === "text" ? "private text must not pass" : new Uint8Array(kind === "empty" ? 0 : 65536));
  await expect.poll(() => desktop.closeCode).toBe(kind === "text" ? 1003 : 1009);
  expect(phone.binary).toHaveLength(0);
});

it("allows a full 65535-byte Noise frame but caps per-session total bytes", async () => {
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  const phone = await connect(pair, "phone");
  desktop.ws.send(new Uint8Array(65535));
  await expect.poll(() => phone.binary.length).toBe(1);
  expect(phone.binary[0]!.byteLength).toBe(65535);
  await runInDurableObject(stub(pair), (_obj, state) => {
    const ws = state.getWebSockets(pair.pairId).find(ws => ws.deserializeAttachment().role === "desktop")!;
    const attachment = ws.deserializeAttachment();
    attachment.bytes = MAX_SESSION_BYTES;
    ws.serializeAttachment(attachment);
  });
  await evictDurableObject(stub(pair));
  desktop.ws.send(new Uint8Array([1]));
  await expect.poll(() => desktop.closeCode).toBe(4008);
  expect(phone.binary).toHaveLength(1);
});

it("persists the 100-token role bucket across hibernation and replacement", async () => {
  vi.spyOn(Date, "now").mockReturnValue(Date.now());
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  const phone = await connect(pair, "phone");
  for (let n = 0; n < 100; n++) desktop.ws.send(new Uint8Array([n]));
  await expect.poll(() => phone.binary.length, { timeout: 5000 }).toBe(100);
  await evictDurableObject(stub(pair));
  const replacement = await connect(pair, "desktop");
  const replacementPeer = await connect(pair, "phone");
  replacement.ws.send(new Uint8Array([101]));
  await expect.poll(() => replacement.closeCode).toBe(4008);
  expect(replacementPeer.binary).toHaveLength(0);
});

it("uses a one-shot alarm to expire idle sessions", async () => {
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  await runInDurableObject(stub(pair), (_obj, state) => {
    const ws = state.getWebSockets(pair.pairId)[0]!;
    const attachment = ws.deserializeAttachment();
    attachment.deadline = Date.now() - 1;
    ws.serializeAttachment(attachment);
  });
  expect(await runDurableObjectAlarm(stub(pair))).toBe(true);
  await expect.poll(() => desktop.closeCode).toBe(4003);
});

it("refills exactly one role token per 600ms, independently of the opposite role", async () => {
  let now = Date.now();
  vi.spyOn(Date, "now").mockImplementation(() => now);
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  const phone = await connect(pair, "phone");
  for (let n = 0; n < 100; n++) desktop.ws.send(new Uint8Array([n]));
  await expect.poll(() => phone.binary.length, { timeout: 5000 }).toBe(100);
  phone.ws.send(new Uint8Array([9]));
  await expect.poll(() => desktop.binary.length).toBe(1);
  now += 600;
  await evictDurableObject(stub(pair));
  desktop.ws.send(new Uint8Array([100]));
  await expect.poll(() => phone.binary.length).toBe(101);
  desktop.ws.send(new Uint8Array([101]));
  await expect.poll(() => desktop.closeCode).toBe(4008);
  expect(phone.binary).toHaveLength(101);
});

it.each(["generation", "peer", "id"])("fails closed on a mismatched sender %s after hibernation", async field => {
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  const phone = await connect(pair, "phone");
  await runInDurableObject(stub(pair), (_obj, state) => {
    const ws = state.getWebSockets(pair.pairId).find(ws => ws.deserializeAttachment().role === "desktop")!;
    const a = ws.deserializeAttachment();
    a[field] = "0".repeat(32);
    ws.serializeAttachment(a);
  });
  await evictDurableObject(stub(pair));
  desktop.ws.send(new Uint8Array([15]));
  await expect.poll(() => desktop.closeCode).toBe(1012);
  expect(phone.binary).toHaveLength(0);
});

it("denies a stale destination association even when sender association is current", async () => {
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  const phone = await connect(pair, "phone");
  await runInDurableObject(stub(pair), (_obj, state) => {
    const ws = state.getWebSockets(pair.pairId).find(ws => ws.deserializeAttachment().role === "phone")!;
    const a = ws.deserializeAttachment();
    a.generation = "0".repeat(32);
    ws.serializeAttachment(a);
  });
  await evictDurableObject(stub(pair));
  desktop.ws.send(new Uint8Array([16]));
  await expect.poll(() => desktop.closeCode).toBe(1012);
  expect(phone.binary).toHaveLength(0);
});

it("does not allocate application tables for guessed account routes", async () => {
  const pair = await provision();
  pair.accountKey = "0".repeat(64);
  expect((await upgrade(pair, "desktop")).status).toBe(401);
  const tables = await runInDurableObject(stub(pair), (_obj, state) => state.storage.sql.exec("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE '_cf_%'").toArray());
  expect(tables).toEqual([]);
});

it("bounds accepted sockets at twenty under parallel replacement churn", async () => {
  const pairs = [];
  for (let n = 0; n < 10; n++) {
    const pair = await provision();
    pairs.push(pair);
    await connect(pair, "desktop");
    await connect(pair, "phone");
  }
  const pair = pairs[0]!;
  await Promise.all(Array.from({ length: 6 }, async () => {
    const response = await upgrade(pair, "desktop");
    expect([101, 429]).toContain(response.status);
    if (response.webSocket) { response.webSocket.accept(); response.webSocket.close(1000, "test_complete"); }
  }));
  expect(await runInDurableObject(stub(pair), (_obj, state) => state.getWebSockets().length)).toBeLessThanOrEqual(20);
});

it("handles simulated storage/quota failure without forwarding or logging private material", async () => {
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  const phone = await connect(pair, "phone");
  const errors = vi.spyOn(console, "error");
  const logs = vi.spyOn(console, "log");
  await runInDurableObject(stub(pair), (obj, state) => {
    const ws = state.getWebSockets(pair.pairId).find(ws => ws.deserializeAttachment().role === "desktop")!;
    const exec = vi.spyOn(state.storage.sql, "exec").mockImplementation(() => { throw new Error("synthetic_quota_failure"); });
    try { obj.webSocketMessage(ws, new Uint8Array([91, 92]).buffer); } finally { exec.mockRestore(); }
  });
  await expect.poll(() => desktop.closeCode).toBe(1012);
  expect(phone.binary).toHaveLength(0);
  expect(errors).not.toHaveBeenCalled();
  expect(logs).not.toHaveBeenCalled();
});

it("caps total session message count even after hibernation", async () => {
  const pair = await provision();
  const desktop = await connect(pair, "desktop");
  const phone = await connect(pair, "phone");
  await runInDurableObject(stub(pair), (_obj, state) => {
    const ws = state.getWebSockets(pair.pairId).find(ws => ws.deserializeAttachment().role === "desktop")!;
    const a = ws.deserializeAttachment();
    a.messages = MAX_SESSION_MESSAGES;
    ws.serializeAttachment(a);
  });
  await evictDurableObject(stub(pair));
  desktop.ws.send(new Uint8Array([17]));
  await expect.poll(() => desktop.closeCode).toBe(4008);
  expect(phone.binary).toHaveLength(0);
});
