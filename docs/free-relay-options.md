# Free-first relay hosting

Reviewed: 2026-09-05. This is a deployment decision record, not a live service.

## Budget rule

Use free services and open-source tools by default. Prefer ongoing free tiers
over expiring trials. Never enable billing, buy a domain, or start a trial that
automatically charges without explicit approval. A quota error must be visible
to the user; it must not trigger a paid upgrade.

## Options

| Option | Benefit | Limitation | Decision |
| --- | --- | --- | --- |
| Existing LAN/hotspot | No server or hosting cost | Cannot cross client isolation or NAT on unrelated networks | Keep as default |
| Cloudflare Workers Free + SQLite Durable Objects | Public WSS endpoint and persistent pair coordination; hibernating sockets | Quotas, provider outages; requires a Workers-compatible relay rather than deploying the Actix binary | Preferred free relay candidate |
| Render Free web service | Can host the existing Docker service after hardening | Idle spin-down, ephemeral filesystem, cold starts; unsuitable as the only durable state store | Development comparison, not our reliability baseline |

Private DNS only resolves names. It does not create a path through firewalls.
Both clients should open outbound WSS connections on port 443 to the relay.
An administrator can still block the service or require a captive-portal login.
Do not promise connection on every network or circumvent university policy.

## Free account setup

1. Create a Cloudflare account at https://dash.cloudflare.com/sign-up and verify
   the email yourself. Keep passwords, verification codes, and recovery codes private.
2. Open Workers & Pages and select the Workers Free plan. A purchased domain is
   not needed for initial testing; use the provided workers.dev subdomain.
3. Tell the developer only that the free account is ready and which workers.dev
   subdomain you chose. Do not paste API tokens or account credentials into chat.
4. Once reviewed deployment code exists, authenticate Wrangler in your own browser
   on this PC. This lets deployment happen without sharing your password.
5. Verify that the account is still on Free before deployment. If the dashboard
   requires a paid plan, stop rather than approving the upgrade.

Current code cannot be safely deployed just by creating this account. The Actix
relay must not receive the end-to-end encryption secret. A Workers implementation
must preserve the client protocol without importing that flaw.

## Required implementation and acceptance gates

- Separate per-role relay credentials from device-only encryption keys. Avoid
  credentials in URLs or logs. Authenticate both roles and support revocation.
- Authenticate the session handshake, reject replayed encrypted frames, and
  preserve ACK/retry deduplication across reconnects and relay restarts.
- Store only minimal pair metadata and bounded encrypted queues with expiry.
  Keep plaintext notification content and app inventory out of relay storage.
- Use SQLite-backed Durable Objects and the WebSocket Hibernation API. Do not
  keep objects awake with periodic JavaScript timers. Enforce payload/rate limits.
- Test expiry, wrong-role access, stolen/revoked credentials, quota exhaustion,
  offline recipients, restarts, and real Android-to-Windows different-network sync.
- Keep the current cloud-mode block until both clients and relay pass these gates.

## Provider documentation

Cloudflare Free currently includes SQLite-backed Durable Objects. Exhausted free
limits fail operations rather than silently granting unlimited capacity. Its
hibernation API can keep idle WebSockets connected without active execution.

- https://developers.cloudflare.com/durable-objects/platform/pricing/
- https://developers.cloudflare.com/durable-objects/best-practices/websockets/
- https://developers.cloudflare.com/workers/configuration/routing/workers-dev/
- https://render.com/docs/free

Recheck these limits before deploying; free tiers are not permanent guarantees.
Cloudflare describes workers.dev as suitable for personal/hobby use and recommends
a route or custom domain for business-critical production. Our no-cost hostname
is therefore a testing option, not an uptime guarantee for a commercial launch.
