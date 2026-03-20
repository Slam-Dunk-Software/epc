# EPC — Extremely Personal Cloud

**EPC is itself an EPS.** It's a personal cloud harness — functional out of the box,
but deliberately incomplete. The services you deploy into it are the ports. The default
state is a starting point, not a destination.

A lightweight personal PaaS that runs EPS packages as persistent services on your own
hardware, instantly accessible on all your devices via Tailscale.

---

## The Three-Layer Stack

```
EPS   — the app format      (eps.toml, CUSTOMIZE.md, intentional ports)
EPM   — the package manager (install, search, publish)
EPC   — the runtime         (deploy, ps, logs, stop)
```

EPS defines what an app is. EPM manages where it lives on disk. EPC makes it run —
and Tailscale makes it reachable everywhere.

---

## The Idea

You have a Mac, a Pi, or a cheap VPS. Install Tailscale on it. Now it has a stable
hostname — `my-machine.tail.net` — reachable from your phone, your laptop, anywhere,
without port forwarding, without dynamic DNS, without a cloud middleman.

EPC turns that machine into your personal cloud:

```
epc serve tech_talker
# → installs via epm
# → starts as a persistent daemon
# → binds to port 8080
# → immediately reachable at https://my-machine.tail.net:8080
```

Bookmark it in Safari. Add it to your phone's home screen. It's just a website —
one that happens to run entirely on hardware you own.

---

## Why Tailscale

Tailscale is the missing piece that makes personal cloud feel like consumer software.
It handles the hard parts — NAT traversal, peer-to-peer routing, stable hostnames,
device auth — so EPC doesn't have to.

Without Tailscale, "accessible from your phone" requires:
- A static IP or dynamic DNS service
- Port forwarding rules on your router
- A reverse proxy with TLS certificates
- Firewall configuration

With Tailscale, it's just a URL. EPC is designed around this assumption.
Tailscale is a first-class dependency, not an optional integration.

EPC reads your Tailscale node name at startup and uses it as the base for all service
URLs. `epc ps` shows you exactly where each service lives:

```
NAME          PORT   URL
tech_talker   8080   https://my-machine.tail.net:8080
pi            3000   https://my-machine.tail.net:3000
```

---

## What EPC Provides

- **Process lifecycle** — start, stop, restart, crash recovery
- **Port allocation** — no conflicts between installed services
- **Log streaming** — `epc logs <name>` tails stdout/stderr
- **Tailscale integration** — reads node name via `tailscale status`, surfaces URLs in `epc ps`
- **Service registry** — a local state file tracking what's running and where

---

## What EPSs Need to Declare

For a package to be deployable via EPC, its `eps.toml` should include:

```toml
[service]
enabled = true
start   = "./run.sh serve"
port    = 8080   # default; epc may override to avoid conflicts
```

This is a proposed extension to the eps.toml spec (not yet ADR'd).

---

## Commands

| Command              | Description                                          |
|----------------------|------------------------------------------------------|
| `epc serve <spec>`  | Install + start an EPS as a persistent daemon        |
| `epc ps`             | List running services with ports and Tailscale URLs  |
| `epc logs <name>`    | Tail stdout/stderr for a service                     |
| `epc stop <name>`    | Stop a running service                               |

---

## Open Questions

- macOS launchd / Linux systemd integration, or a custom supervisor loop?
- Should `epc` embed a reverse proxy so all services share one HTTPS port with path routing
  (e.g. `https://my-machine.tail.net/tech_talker`)? Tailscale's HTTPS certs make this
  particularly clean.
- Is EPC itself an EPS? (Almost certainly yes — dogfooding opportunity.)
- How does EPC interact with EPM's install lifecycle (CAS, git SHA pinning)?
- Tailscale Funnel support for making specific services public (opt-in, explicit)?
