import { AccountRelay } from "./account";
import { FirebaseVerifier, PROJECT } from "./firebase";
import { bearer, boundedText, failure, hash, json, RelayError } from "./http";

export interface Env { ACCOUNTS: DurableObjectNamespace<AccountRelay> }
export { AccountRelay };
const identity = new FirebaseVerifier();

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    try {
      const url = new URL(request.url);
      if (url.protocol !== "https:") throw new RelayError(400, "https_required");
      if (url.search) throw new RelayError(400, "query_not_allowed");
      if (url.pathname === "/health" && request.method === "GET") return json({ status: "ok", protocol: 1 });
      const socket = /^\/v1\/socket\/([a-f0-9]{64})\/([a-f0-9]{32})\/(desktop|phone)$/.exec(url.pathname);
      if (socket) {
        if (request.method !== "GET") throw new RelayError(405, "method_not_allowed");
        const value = bearer(request, 43);
        if (!/^[A-Za-z0-9_-]{43}$/.test(value)) throw new RelayError(401, "unauthorized");
        if (request.headers.get("Upgrade")?.toLowerCase() !== "websocket") throw new RelayError(426, "websocket_required");
        return await env.ACCOUNTS.get(env.ACCOUNTS.idFromName(socket[1]!)).fetch(new Request(url, {
          headers: { Upgrade: "websocket", Authorization: `Bearer ${value}` },
        }));
      }
      const owner = /^\/v1\/pairs(?:\/([a-f0-9]{32}))?$/.exec(url.pathname);
      if (!owner) throw new RelayError(404, "not_found");
      if (owner[1] ? request.method !== "DELETE" : !["GET", "POST"].includes(request.method)) throw new RelayError(405, "method_not_allowed");
      const uid = await identity.verify(bearer(request));
      if (request.method === "POST") {
        const text = await boundedText(request.body, 1024);
        if (text) {
          let body: unknown;
          try { body = JSON.parse(text); } catch { throw new RelayError(400, "invalid_request"); }
          if (!body || typeof body !== "object" || Array.isArray(body) || Object.keys(body).length) throw new RelayError(400, "invalid_request");
        }
      }
      const accountKey = await hash(JSON.stringify(["focusbridge-relay-account-v1", PROJECT, uid]));
      return await env.ACCOUNTS.get(env.ACCOUNTS.idFromName(accountKey)).manage(accountKey, request.method, owner[1]);
    } catch (error) { return failure(error); }
  },
} satisfies ExportedHandler<Env>;
