export class RelayError extends Error {
  constructor(readonly status: number, readonly code: string) { super(code); }
}

export function json(value: unknown, status = 200): Response {
  return new Response(status === 204 ? null : JSON.stringify(value), {
    status,
    headers: { "Content-Type": "application/json", "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff" },
  });
}

export function failure(error: unknown): Response {
  return error instanceof RelayError ? json({ error: error.code }, error.status)
    : json({ error: "temporarily_unavailable" }, 503);
}

export function bearer(request: Request, max = 8192): string {
  const value = request.headers.get("Authorization");
  if (!value || value.length > max + 7 || !/^Bearer [A-Za-z0-9_.-]+$/i.test(value)) {
    throw new RelayError(401, "unauthorized");
  }
  return value.slice(7);
}

export async function boundedText(body: ReadableStream<Uint8Array> | null, max: number): Promise<string> {
  if (!body) return "";
  const reader = body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > max) {
        await reader.cancel();
        throw new RelayError(413, "request_too_large");
      }
      chunks.push(value);
    }
  } finally { reader.releaseLock(); }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
  return new TextDecoder("utf-8", { fatal: true, ignoreBOM: false }).decode(bytes);
}

export async function hash(value: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
  return Array.from(new Uint8Array(digest), n => n.toString(16).padStart(2, "0")).join("");
}

export function randomHex(bytes: number): string {
  return Array.from(crypto.getRandomValues(new Uint8Array(bytes)), n => n.toString(16).padStart(2, "0")).join("");
}

export function capability(): string {
  return btoa(String.fromCharCode(...crypto.getRandomValues(new Uint8Array(32))))
    .replaceAll("+", "-").replaceAll("/", "_").replaceAll("=", "");
}

export function capabilityHash(account: string, pair: string, role: string, value: string) {
  return hash(JSON.stringify(["focusbridge-relay-cap-v1", account, pair, role, value]));
}
