# Free-first relay hosting

Reviewed: 2026-09-05. The relay is deployed for verification; app integration is not release-ready.

## Deployment checkpoint

The authenticated Worker is deployed at
`https://focusbridge-relay.focusbridge.workers.dev`.
Version: `e28840cb-8d8f-40e9-8bf9-878fbee42cd7`.
Public `/health` returned `{"status":"ok","protocol":1}` and anonymous
`POST /v1/pairs` returned HTTP 401. These checks do not verify authenticated
device traffic, end-to-end encryption, cross-network pairing, or endurance.
Private relay traffic remains blocked in app clients pending those gates.

Deployment required `workers_scripts:write` and `workers_routes:write` in
addition to the original account/user/generic Worker scopes. Browser OAuth
completed with credentials retained in the Windows keyring. No billing upgrade
or paid binding was requested. Historical setup notes below describe the
pre-deployment state and must not be mistaken for current deployment status.

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
   subdomain you chose. If no subdomain is shown yet, just confirm account setup.
   Do not paste API tokens or account credentials into chat. You do not need to
   deploy a Hello World template or connect the GitHub repository at this step.
4. Once reviewed deployment code exists, authenticate Wrangler in your own browser
   on this PC. This lets deployment happen without sharing your password.
5. Verify that the account is still on Free before deployment. If the dashboard
   requires a paid plan, stop rather than approving the upgrade.

Current code cannot be safely deployed just by creating this account. The Actix
relay must not receive the end-to-end encryption secret. A Workers implementation
must preserve the client protocol without importing that flaw.

## This workspace's account setup

On 2026-09-05 the owner reported creating the account with the account subdomain
`focusbridge.workers.dev`. The account ID is kept in the ignored local file
`tools/cloudflare/.env.local`, not compiled into either app. These identifiers
are not API credentials and cannot authorize a deployment on their own.

The account subdomain is not a running relay. A Worker named `focusbridge-relay`
would have the address `focusbridge-relay.focusbridge.workers.dev` after a
successful deployment. Do not enter that address in pairing settings yet.

From `focusbridge/tools/cloudflare`:

```powershell
pnpm install --frozen-lockfile
pnpm run login
pnpm run whoami
```

Use `pnpm run`, not `pnpm login` or `pnpm whoami`: those are package-registry
commands rather than these Cloudflare scripts.

The login script requests only `account:read` and `user:read` for account
verification; Wrangler adds `offline_access` for refreshing that authorization.
Approve it in your own browser. It cannot deploy the relay;
deployment scopes will be requested only when reviewed relay code is ready.
Do not send OAuth callback URLs, verification codes, or tokens in chat.

Wrangler is pinned locally. `CLOUDFLARE_AUTH_USE_KEYRING=true` requires protected
credential storage and prevents a silent plaintext fallback. On Windows the
encryption key is held in Credential Manager. Its optional keyring binding may
need a one-time installation. Never use `wrangler auth token` in shared logs.
Telemetry is disabled for these scripts. Login does not enable a paid plan.

Verified on this PC on 2026-09-05: Wrangler 4.129.0 login succeeded, `whoami`
matched the supplied account ID, and reported an encrypted credential file with
its key in Windows Credential Manager. `whoami` warns about missing write
scopes because this login is intentionally read-only; do not blindly grant all
scopes to silence it. Frozen/offline installation passed, and npm's advisory
audit reported zero known vulnerabilities for this tooling lockfile. This is
not a security audit of the FocusBridge apps or a verification of the Free plan.

Before any deployment, separately check Workers & Pages shows the Free plan.
Account authentication alone does not establish plan status or network readiness.
There is deliberately no deploy script or placeholder public relay in this
tooling package. Both clients must retain the private-cloud traffic block until
the acceptance gates below pass.

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
- https://developers.cloudflare.com/workers/get-started/dashboard/
- https://developers.cloudflare.com/workers/wrangler/commands/general/#login
- https://render.com/docs/free

Recheck these limits before deploying; free tiers are not permanent guarantees.
Cloudflare describes workers.dev as suitable for personal/hobby use and recommends
a route or custom domain for business-critical production. Our no-cost hostname
is therefore a testing option, not an uptime guarantee for a commercial launch.
