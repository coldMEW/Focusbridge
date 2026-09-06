import { env, exports } from "cloudflare:workers";
import { importPKCS8, SignJWT } from "jose";
import keyPem from "./fixtures/key.pem?raw";
import type { Env } from "../src/index";
import { afterEach, beforeEach, vi } from "vitest";

let ownerId = "";
beforeEach(() => { ownerId = crypto.randomUUID(); });
const sockets: WebSocket[] = [];
afterEach(() => {
  vi.restoreAllMocks();
  for (const socket of sockets.splice(0)) {
    if (socket.readyState === WebSocket.OPEN) socket.close(1000, "test_complete");
  }
});

export const bindings = env as Env;
export const api = (path: string, init?: RequestInit) => exports.default.fetch(`https://relay.test${path}`, init);
let signingKey: Promise<CryptoKey> | undefined;
export async function token(claims: Record<string, unknown> = {}, header: Record<string, unknown> = {}) {
  signingKey ??= importPKCS8(keyPem, "RS256");
  const now = Math.floor(Date.now() / 1000);
  return new SignJWT({
    iss: "https://securetoken.google.com/foucsbridge", aud: "foucsbridge",
    sub: ownerId, iat: now - 1, exp: now + 3600, auth_time: now - 30,
    email: "synthetic@example.invalid", email_verified: true, ...claims,
  }).setProtectedHeader({ alg: "RS256", kid: "fixture-key", ...header }).sign(await signingKey);
}
export async function ownerRequest(path: string, method = "GET", jwt?: string, body?: string) {
  return api(path, { method, headers: { Authorization: `Bearer ${jwt ?? await token({ sub: ownerId })}` }, body });
}
export interface Pair {
  accountKey: string;
  pairId: string;
  createdAt: number;
  expiresAt: number;
  capabilities: { desktop: string; phone: string };
}
export async function provision(jwt?: string): Promise<Pair> {
  const response = await ownerRequest("/v1/pairs", "POST", jwt);
  if (response.status !== 201) throw new Error(`Provision returned ${response.status}`);
  return response.json<Pair>();
}
export function stub(pair: Pair) {
  return bindings.ACCOUNTS.get(bindings.ACCOUNTS.idFromName(pair.accountKey));
}

export function socketPath(pair: Pair, role: string) {
  return `/v1/socket/${pair.accountKey}/${pair.pairId}/${role}`;
}
export function upgrade(pair: Pair, role: "desktop" | "phone", cap = pair.capabilities[role]) {
  return api(socketPath(pair, role), { headers: { Upgrade: "websocket", Authorization: `Bearer ${cap}` } });
}
export async function connect(pair: Pair, role: "desktop" | "phone") {
  const response = await upgrade(pair, role);
  if (response.status !== 101 || !response.webSocket) throw new Error(`Upgrade returned ${response.status}`);
  const ws = response.webSocket;
  ws.binaryType = "arraybuffer";
  const messages: (string | ArrayBuffer)[] = [];
  let closeCode: number | undefined;
  ws.addEventListener("message", event => { messages.push(event.data as string | ArrayBuffer); });
  ws.addEventListener("close", event => { closeCode = event.code; });
  ws.accept();
  sockets.push(ws);
  return { ws, messages, get closeCode() { return closeCode; },
    get binary() { return messages.filter((m): m is ArrayBuffer => m instanceof ArrayBuffer); },
    get controls() { return messages.filter((m): m is string => typeof m === "string").map(m => JSON.parse(m)); } };
}
