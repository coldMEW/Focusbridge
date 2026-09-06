import { decodeProtectedHeader, importX509, jwtVerify } from "jose";
import { boundedText, RelayError } from "./http";

export const PROJECT = "foucsbridge";
const ISSUER = `https://securetoken.google.com/${PROJECT}`;
const CERTS = "https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com";

export class FirebaseVerifier {
  private certificates = new Map<string, string>();
  private keys = new Map<string, Promise<CryptoKey>>();
  private expires = 0;
  private retryAt = 0;
  private pending?: Promise<void>;

  constructor(private readonly fetcher: typeof fetch = (input, init) => fetch(input, init), private readonly now: () => number = Date.now) {}

  private async refresh(): Promise<void> {
    if (this.now() < this.expires) return;
    if (this.pending) return this.pending;
    if (this.now() < this.retryAt) throw new RelayError(503, "identity_unavailable");
    this.pending = this.load();
    try { await this.pending; } finally { this.pending = undefined; }
  }

  private async load(): Promise<void> {
    const started = this.now();
    try {
      const response = await this.fetcher(CERTS, { redirect: "manual", signal: AbortSignal.timeout(5000) });
      if (!response.ok) throw new Error();
      const cache = response.headers.get("Cache-Control") ?? "";
      const maxAge = /(?:^|,)\s*max-age=(\d+)\s*(?:,|$)/i.exec(cache)?.[1];
      const age = Number(response.headers.get("Age") ?? "0");
      if (!maxAge || /(?:no-store|no-cache)/i.test(cache) || !Number.isFinite(age) || age < 0) throw new Error();
      const date = Date.parse(response.headers.get("Date") ?? "");
      const elapsed = Math.max(age * 1000, Number.isFinite(date) ? started - date : 0);
      const expires = started + Number(maxAge) * 1000 - elapsed;
      if (!Number.isSafeInteger(expires) || expires <= this.now()) throw new Error();
      const values: unknown = JSON.parse(await boundedText(response.body, 65536));
      if (!values || typeof values !== "object" || Array.isArray(values)) throw new Error();
      const entries = Object.entries(values);
      if (!entries.length || entries.length > 16 || entries.some(([kid, pem]) =>
        !kid || kid.length > 128 || typeof pem !== "string" || pem.length > 8192 || !pem.startsWith("-----BEGIN CERTIFICATE-----"))) throw new Error();
      this.certificates = new Map(entries as [string, string][]);
      this.keys.clear();
      this.expires = expires;
    } catch {
      this.certificates.clear();
      this.keys.clear();
      this.expires = 0;
      this.retryAt = this.now() + 30000;
      throw new RelayError(503, "identity_unavailable");
    }
  }

  async verify(token: string): Promise<string> {
    try {
      if (token.length > 8192) throw new Error();
      const header = decodeProtectedHeader(token);
      if (header.alg !== "RS256" || typeof header.kid !== "string" || !header.kid || header.kid.length > 128 || header.crit) throw new Error();
      await this.refresh();
      const pem = this.certificates.get(header.kid);
      if (!pem) throw new Error();
      let key = this.keys.get(header.kid);
      if (!key) { key = importX509(pem, "RS256"); this.keys.set(header.kid, key); }
      const now = Math.floor(this.now() / 1000);
      const { payload: p } = await jwtVerify(token, await key, {
        algorithms: ["RS256"], issuer: ISSUER, audience: PROJECT,
        requiredClaims: ["exp", "iat", "auth_time", "sub"], currentDate: new Date(this.now()),
      });
      // Account routing is project-scoped, not tenant-scoped. Accepting a tenant
      // token here could alias a tenant-local UID with an existing account.
      if (p.firebase !== undefined && (p.firebase === null || typeof p.firebase !== "object" ||
        Array.isArray(p.firebase) || "tenant" in p.firebase)) throw new Error();
      if (p.aud !== PROJECT || typeof p.sub !== "string" || !p.sub || p.sub.length > 128 ||
        !Number.isSafeInteger(p.exp) || !Number.isSafeInteger(p.iat) || !Number.isSafeInteger(p.auth_time) ||
        (p.exp as number) <= now || (p.iat as number) > now || (p.iat as number) < 0 ||
        (p.auth_time as number) > (p.iat as number) || (p.auth_time as number) < 0 ||
        (p.exp as number) <= (p.iat as number) || p.email_verified !== true ||
        typeof p.email !== "string" || !p.email || p.email.length > 320) throw new Error();
      return p.sub;
    } catch (error) {
      if (error instanceof RelayError) throw error;
      throw new RelayError(401, "unauthorized");
    }
  }
}
