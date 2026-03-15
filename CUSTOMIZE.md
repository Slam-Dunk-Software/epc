# Customizing EPC

EPC is an EPS — Extremely Personal Software. It ships as a functional personal cloud
harness, but the default state is a starting point, not a destination. The services you
deploy into it are what make it yours.

This document describes EPC's extension points and how to use them.

---

## Port 1: Services

**This is the primary port.** EPC exists to run services. Without any deployed services
it's just a process supervisor watching an empty list.

Deploy an EPS package as a persistent service:

```bash
epc deploy tech_talker        # latest version
epc deploy tech_talker@1.2.0  # pinned version
```

EPC reads `[service]` from the package's `eps.toml` to know how to start it and what
port it wants. It then:
1. Installs the package via `epm` (if not already installed)
2. Allocates a port (respects the package's default, overrides if conflicted)
3. Starts the process and registers it in `~/.epc/services.toml`
4. Surfaces the Tailscale URL in `epc ps`

Your `~/.epc/services.toml` is the live registry of what's running. You can edit it
directly if you need to adjust a port or rename a service entry.

---

## Port 2: Supervisor

EPC's default process supervisor is a built-in tokio-based loop. It watches processes,
restarts on crash, and streams logs.

If you want to use your OS's native process manager instead, set `supervisor` in `eps.toml`:

```toml
[ports]
supervisor = "launchd"   # macOS
supervisor = "systemd"   # Linux
```

With `launchd` or `systemd`, EPC generates the appropriate unit/plist files and registers
them with the OS. Your services survive reboots automatically.

---

## Port 3: Dashboard

EPC ships without a web dashboard. The CLI (`epc ps`) is the default interface.

To add a dashboard, install a dashboard EPS and register it:

```bash
epc deploy portboard   # example dashboard EPS (hypothetical)
```

Or build your own. A dashboard EPS is just a web app that reads `~/.epc/services.toml`
and renders links. EPC exposes that file as the source of truth.

---

## The State File

`~/.epc/services.toml` is the single source of truth for what EPC is managing:

```toml
[services.tech_talker]
spec    = "tech_talker@1.2.0"
port    = 8080
pid     = 12345
status  = "running"
started = "2026-02-27T10:00:00Z"

[services.pi]
spec    = "pi@0.3.1"
port    = 3000
pid     = 12346
status  = "running"
started = "2026-02-27T10:01:00Z"
```

You own this file. EPC reads and writes it, but it's plain TOML — inspect it, back it
up, or sync it across machines however you like.
