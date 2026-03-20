# ADR-0002: Tailscale as the Required Networking Layer

- **Date:** 2026-02-27
- **Status:** Accepted (amended 2026-02-28 — see Service URL Format below)

## Context

EPC's core promise is that you can run personal software on your own hardware and access
it from any of your devices — phone, laptop, tablet — without a cloud subscription or
complex networking setup.

Delivering on this promise requires solving the "last mile" problem: making a service
running on your Mac or Pi reachable outside your local network without port forwarding,
static IPs, or dynamic DNS.

Alternatives considered:

| Approach             | Notes |
|----------------------|-------|
| Raw port forwarding  | Requires router access, static IP or DDNS, fragile |
| Cloudflare Tunnel    | Free tier available, but adds a third-party middleman to all traffic |
| Wireguard (manual)   | Technically sound but complex to configure; no device auth story |
| Tailscale            | Zero-config, stable hostnames, peer-to-peer, device auth, HTTPS certs |
| LAN-only             | No mobile access unless you're home; defeats the personal cloud vision |

## Decision

Tailscale is a first-class dependency of EPC, not an optional integration. EPC assumes
Tailscale is installed and the machine is a member of a Tailnet.

Concretely:
- EPC reads the Tailscale node's IPv4 address at deploy/ps time via `tailscale status --json`
- `epc ps` displays URLs using the Tailscale IP (e.g. `http://100.78.103.79:8080`) for every
  running service — not the DNS name (see Service URL Format below)
- Documentation and onboarding assume Tailscale is already set up
- EPC falls back to `localhost` and continues rather than hard-failing if Tailscale is not running

### Service URL Format

EPC surfaces service URLs using the **Tailscale IPv4 address** (`TailscaleIPs[0]`, the
`100.x.x.x` CGNAT address) rather than the DNS name (`DNSName`, e.g. `machine.tail.net`).

**Why IP over DNS name:**
- The DNS name is publicly resolvable via standard DNS. Exposing it in logs, `epc ps` output,
  or shared screenshots leaks network topology to anyone who reads them.
- The Tailscale IP (`100.x.x.x`) is meaningful only inside the tailnet. It cannot be resolved
  or reached from the public internet, which matches the personal-cloud privacy model.
- Both the IP and the DNS name provide the same access — any device in the tailnet can use
  either. The IP is strictly safer for surfacing in user-visible output.

```
epc ps output:
  simple_todo   8765   100.78.103.79   running   http://100.78.103.79:8765
```

The Tailscale IP is obtained from `Self.TailscaleIPs` in the `tailscale status --json`
output. The first entry starting with `100.` is used (IPv4 CGNAT); the IPv6 address
(`fd7a:...`) is ignored for URL formatting purposes.

This is not a "you must use Tailscale" philosophical stance — it's a "we solve the
hard problem by picking a solution" pragmatic stance. Tailscale is free for personal
use, open-source (client), and does the right thing by default.

## Consequences

**Positive:**
- EPC can surface real, working URLs immediately after `epc serve` — no user configuration
- HTTPS is available out of the box via Tailscale's cert provisioning (`tailscale cert`)
- Device auth is handled by Tailscale; EPC inherits it for free
- Tailscale Funnel is a natural opt-in path for making individual services public
- The pitch is clean: "Install Tailscale, install EPC, you have a personal cloud"

**Negative:**
- Requires Tailscale to be installed and running (additional onboarding step)
- Adds a dependency on Tailscale's infrastructure for coordination (though data is P2P)
- Users who already have a VPN/networking setup may find this opinionated
