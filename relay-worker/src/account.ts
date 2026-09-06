import { DurableObject } from "cloudflare:workers";
import { bearer, capability, capabilityHash, failure, json, randomHex, RelayError } from "./http";
import type { Env } from "./index";

export const PAIR_TTL_MS = 30 * 86400000;
export const MAX_FRAME_BYTES = 65535;
// Match the device session lifetime/budgets. The persisted per-role token bucket
// still limits traffic; normal inventories must not trigger periodic disconnects.
export const MAX_SESSION_BYTES = 1024 * 1024 * 1024;
export const MAX_SESSION_MESSAGES = 1_000_000;
export const SESSION_MS = 24 * 60 * 60 * 1000;
type Role = "desktop" | "phone";
type PairRow = {
  pair_id: string; created_at: number; expires_at: number; generation: string | null;
} & Record<`${Role}_hash`, string>
  & Record<`${Role}_socket`, string | null>
  & Record<`${Role}_expires_at` | `${Role}_cap_generation` | `${Role}_credits` | `${Role}_budget_at`, number>;
type Attachment = {
  version: 1; pairId: string; role: Role; id: string; peer: string | null;
  generation: string | null; capGeneration: number; capHash: string;
  deadline: number; bytes: number; messages: number;
};
const opposite = (role: Role): Role => role === "desktop" ? "phone" : "desktop";
const isRole = (role: unknown): role is Role => role === "desktop" || role === "phone";

export class AccountRelay extends DurableObject<Env> {
  private initialized: boolean;

  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env);
    // Unknown socket routes must not create durable rows or tables.
    this.initialized = ctx.storage.sql.exec("SELECT name FROM sqlite_master WHERE type='table' AND name='pairs'").toArray().length > 0;
    ctx.setHibernatableWebSocketEventTimeout(5000);
  }

  private initialize() {
    if (this.initialized) return;
    this.ctx.storage.sql.exec(`CREATE TABLE pairs (
      pair_id TEXT PRIMARY KEY, created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL,
      desktop_hash TEXT NOT NULL, phone_hash TEXT NOT NULL,
      desktop_socket TEXT, phone_socket TEXT, generation TEXT,
      desktop_expires_at INTEGER NOT NULL, phone_expires_at INTEGER NOT NULL,
      desktop_cap_generation INTEGER NOT NULL DEFAULT 1, phone_cap_generation INTEGER NOT NULL DEFAULT 1,
      desktop_credits INTEGER NOT NULL DEFAULT 60000, phone_credits INTEGER NOT NULL DEFAULT 60000,
      desktop_budget_at INTEGER NOT NULL DEFAULT 0, phone_budget_at INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE budgets (name TEXT PRIMARY KEY, start INTEGER NOT NULL, count INTEGER NOT NULL);`);
    this.initialized = true;
  }

  async manage(accountKey: string, method: string, pairId?: string): Promise<Response> {
    try {
      if (this.env.ACCOUNTS.idFromName(accountKey).toString() !== this.ctx.id.toString()) throw new RelayError(403, "forbidden");
      if (!this.initialized && method !== "POST") {
        return method === "GET" ? json({ accountKey, pairs: [] }) : json({ error: "not_found" }, 404);
      }
      this.initialize();
      const now = Date.now();
      if (method === "GET") {
        const pairs = this.ctx.storage.sql.exec("SELECT pair_id AS pairId, created_at AS createdAt, expires_at AS expiresAt FROM pairs WHERE expires_at > ? ORDER BY created_at, pair_id", now).toArray();
        return json({ accountKey, pairs });
      }
      if (method === "DELETE") {
        this.retire(pairId!, 4003);
        const rows = this.ctx.storage.sql.exec("DELETE FROM pairs WHERE pair_id = ? RETURNING pair_id", pairId!).toArray();
        await this.schedule();
        return rows.length ? json(null, 204) : json({ error: "not_found" }, 404);
      }
      if (method !== "POST") throw new RelayError(405, "method_not_allowed");
      const id = randomHex(16);
      const desktop = capability();
      const phone = capability();
      const desktopHash = await capabilityHash(accountKey, id, "desktop", desktop);
      const phoneHash = await capabilityHash(accountKey, id, "phone", phone);
      const createdAt = Date.now();
      const expiresAt = createdAt + PAIR_TTL_MS;
      this.ctx.storage.transactionSync(() => {
        this.purge(createdAt);
        const count = this.ctx.storage.sql.exec<{ n: number }>("SELECT COUNT(*) AS n FROM pairs").one().n;
        if (count >= 10) throw new RelayError(409, "pair_limit");
        // Keep this budget independent of pair rows so revocation cannot reset it.
        const budget = this.ctx.storage.sql.exec<{ start: number; count: number }>(
          "SELECT start, count FROM budgets WHERE name = 'provision'",
        ).toArray()[0];
        const reset = !budget || createdAt >= budget.start + 3600000;
        if (!reset && budget.count >= 20) throw new RelayError(429, "provision_limit");
        this.ctx.storage.sql.exec(
          "INSERT INTO budgets (name, start, count) VALUES ('provision', ?, ?) ON CONFLICT(name) DO UPDATE SET start = excluded.start, count = excluded.count",
          reset ? createdAt : budget.start, reset ? 1 : budget.count + 1,
        );
        this.ctx.storage.sql.exec("INSERT INTO pairs (pair_id, created_at, expires_at, desktop_hash, phone_hash, desktop_expires_at, phone_expires_at) VALUES (?, ?, ?, ?, ?, ?, ?)", id, createdAt, expiresAt, desktopHash, phoneHash, expiresAt, expiresAt);
      });
      await this.schedule();
      return json({ accountKey, pairId: id, createdAt, expiresAt, capabilities: { desktop, phone } }, 201);
    } catch (error) { return failure(error); }
  }

  private read(pairId: string): PairRow | undefined {
    return this.initialized ? this.ctx.storage.sql.exec<PairRow>("SELECT * FROM pairs WHERE pair_id = ?", pairId).toArray()[0] : undefined;
  }

  private attachment(ws: WebSocket): Attachment | undefined {
    const a = ws.deserializeAttachment() as Attachment | null;
    if (!a || a.version !== 1 || !isRole(a.role) || typeof a.pairId !== "string" || typeof a.id !== "string" ||
      typeof a.capHash !== "string" || !Number.isSafeInteger(a.capGeneration) || !Number.isSafeInteger(a.deadline) ||
      !Number.isSafeInteger(a.bytes) || a.bytes < 0 || !Number.isSafeInteger(a.messages) || a.messages < 0) return;
    return a;
  }

  private valid(row: PairRow, a: Attachment, now: number): boolean {
    return row.pair_id === a.pairId && row.expires_at > now && row[`${a.role}_expires_at`] > now &&
      row[`${a.role}_cap_generation`] === a.capGeneration && a.capGeneration > 0 &&
      row[`${a.role}_hash`] === a.capHash && row[`${a.role}_socket`] === a.id && a.deadline > now;
  }

  private socket(pairId: string, id: string | null): WebSocket | undefined {
    return id ? this.ctx.getWebSockets(pairId).find(ws => ws.readyState === WebSocket.OPEN && this.attachment(ws)?.id === id) : undefined;
  }

  private close(ws: WebSocket, code: number) {
    try {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send('{"type":"relay.peer_unavailable"}');
        ws.close(code, "relay_channel_closed");
      }
    } catch { /* A transport failure must not reveal event data. */ }
  }

  private retire(pairId: string, code = 1012) {
    // Fence both endpoints durably BEFORE close callbacks or replacement joins.
    if (this.initialized) this.ctx.storage.sql.exec("UPDATE pairs SET generation = NULL, desktop_socket = NULL, phone_socket = NULL WHERE pair_id = ?", pairId);
    for (const ws of this.ctx.getWebSockets(pairId)) this.close(ws, code);
  }

  private purge(now: number) {
    for (const row of this.ctx.storage.sql.exec<{ pair_id: string }>("SELECT pair_id FROM pairs WHERE expires_at <= ?", now).toArray()) {
      this.retire(row.pair_id, 4003);
      this.ctx.storage.sql.exec("DELETE FROM pairs WHERE pair_id = ?", row.pair_id);
    }
  }

  private schedule(): Promise<void> {
    const deadlines = this.initialized ? this.ctx.storage.sql.exec<{ expires_at: number }>("SELECT expires_at FROM pairs").toArray().map(r => r.expires_at) : [];
    for (const ws of this.ctx.getWebSockets()) {
      const a = this.attachment(ws);
      if (ws.readyState === WebSocket.OPEN && a) deadlines.push(a.deadline);
    }
    return deadlines.length ? this.ctx.storage.setAlarm(Math.max(Date.now() + 1, Math.min(...deadlines))) : this.ctx.storage.deleteAlarm();
  }

  async fetch(request: Request): Promise<Response> {
    let joining: WebSocket | undefined;
    try {
      const url = new URL(request.url);
      const match = /^\/v1\/socket\/([a-f0-9]{64})\/([a-f0-9]{32})\/(desktop|phone)$/.exec(url.pathname);
      if (!match || url.search || request.method !== "GET" || url.protocol !== "https:") throw new RelayError(404, "not_found");
      const accountKey = match[1]!;
      const pairId = match[2]!;
      const role = match[3] as Role;
      if (this.env.ACCOUNTS.idFromName(accountKey).toString() !== this.ctx.id.toString()) throw new RelayError(401, "unauthorized");
      const value = bearer(request, 43);
      if (!/^[A-Za-z0-9_-]{43}$/.test(value)) throw new RelayError(401, "unauthorized");
      if (request.headers.get("Upgrade")?.toLowerCase() !== "websocket") throw new RelayError(426, "websocket_required");
      const digest = await capabilityHash(accountKey, pairId, role, value);
      // All authorization and ownership reads occur AFTER the async hash.
      let row = this.read(pairId);
      const now = Date.now();
      if (!row || row.expires_at <= now || row[`${role}_expires_at`] <= now || row[`${role}_cap_generation`] < 1 ||
        !crypto.subtle.timingSafeEqual(new TextEncoder().encode(row[`${role}_hash`]), new TextEncoder().encode(digest))) throw new RelayError(401, "unauthorized");
      const other = opposite(role);
      let peer = this.socket(pairId, row[`${other}_socket`]);
      let peerAttachment = peer && this.attachment(peer);
      if (row[`${role}_socket`] || row.generation || (row[`${other}_socket`] && (!peerAttachment || !this.valid(row, peerAttachment, now)))) {
        this.retire(pairId);
        row = this.read(pairId)!;
        peer = undefined;
        peerAttachment = undefined;
      }
      // Include CLOSING sockets: rapid replacement must not accumulate buffers.
      if (this.ctx.getWebSockets().length >= 20) throw new RelayError(429, "socket_limit");
      const { 0: client, 1: server } = new WebSocketPair();
      joining = server;
      const id = randomHex(16);
      const generation = peerAttachment ? randomHex(16) : null;
      const a: Attachment = {
        version: 1, pairId, role, id, peer: peerAttachment?.id ?? null, generation,
        capGeneration: row[`${role}_cap_generation`], capHash: digest,
        deadline: Math.min(row.expires_at, row[`${role}_expires_at`], now + SESSION_MS), bytes: 0, messages: 0,
      };
      this.ctx.acceptWebSocket(server, [pairId]);
      server.serializeAttachment(a);
      this.ctx.storage.sql.exec(`UPDATE pairs SET ${role}_socket = ?, generation = ? WHERE pair_id = ?`, id, generation, pairId);
      if (peer && peerAttachment) {
        peerAttachment.peer = id;
        peerAttachment.generation = generation;
        peer.serializeAttachment(peerAttachment);
        const control = JSON.stringify({ type: "relay.peer_ready", generation });
        peer.send(control);
        server.send(control);
      } else server.send('{"type":"relay.peer_unavailable"}');
      await this.schedule();
      return new Response(null, { status: 101, webSocket: client, headers: { "Cache-Control": "no-store" } });
    } catch (error) {
      if (joining) {
        const a = this.attachment(joining);
        if (a) this.retire(a.pairId, 1012);
        else this.close(joining, 1012);
      }
      return failure(error);
    }
  }

  webSocketMessage(ws: WebSocket, message: string | ArrayBuffer): void {
    try {
      const a = this.attachment(ws);
      if (!a || ws.readyState !== WebSocket.OPEN) { this.close(ws, 1012); return; }
      const row = this.read(a.pairId);
      if (!row || row[`${a.role}_socket`] !== a.id) { this.close(ws, 1012); return; }
      const now = Date.now();
      if (!this.valid(row, a, now)) { this.retire(a.pairId, 4003); return; }
      if (typeof message === "string") { this.retire(a.pairId, 1003); return; }
      if (!message.byteLength || message.byteLength > MAX_FRAME_BYTES) { this.retire(a.pairId, 1009); return; }
      const other = opposite(a.role);
      const peer = this.socket(a.pairId, row[`${other}_socket`]);
      const b = peer && this.attachment(peer);
      if (!row.generation || a.generation !== row.generation || a.peer !== row[`${other}_socket`] ||
        !peer || !b || b.role !== other || b.peer !== a.id || b.generation !== row.generation) {
        this.retire(a.pairId); return;
      }
      if (!this.valid(row, b, now)) { this.retire(a.pairId, 4003); return; }
      const at = Math.max(now, row[`${a.role}_budget_at`]);
      const credits = Math.min(60000, row[`${a.role}_credits`] + at - row[`${a.role}_budget_at`]);
      if (credits < 600 || a.bytes + message.byteLength > MAX_SESSION_BYTES || a.messages >= MAX_SESSION_MESSAGES) {
        this.retire(a.pairId, 4008); return;
      }
      // One token per frame, 100-token burst, one token replenished per 600ms.
      this.ctx.storage.sql.exec(`UPDATE pairs SET ${a.role}_credits = ?, ${a.role}_budget_at = ? WHERE pair_id = ?`, credits - 600, at, a.pairId);
      a.messages++;
      a.bytes += message.byteLength;
      ws.serializeAttachment(a);
      peer.send(message);
    } catch {
      // Storage/quota/send errors fail closed, without logging ciphertext or credentials.
      for (const socket of this.ctx.getWebSockets()) this.close(socket, 1012);
    }
  }

  webSocketClose(ws: WebSocket, _code: number, _reason: string, _wasClean: boolean): void {
    const a = this.attachment(ws);
    if (a && this.read(a.pairId)?.[`${a.role}_socket`] === a.id) this.retire(a.pairId);
  }

  webSocketError(ws: WebSocket): void { this.webSocketClose(ws, 1012, "transport_error", false); }

  async alarm(): Promise<void> {
    if (!this.initialized) return;
    const now = Date.now();
    for (const ws of this.ctx.getWebSockets()) {
      const a = this.attachment(ws);
      if (!a) { this.close(ws, 4003); continue; }
      const row = this.read(a.pairId);
      if (row?.[`${a.role}_socket`] === a.id && !this.valid(row, a, now)) this.retire(a.pairId, 4003);
    }
    this.purge(now);
    await this.schedule();
  }
}
