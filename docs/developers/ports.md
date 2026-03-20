# Ports (Extension Points)

EPC has three named extension points. See also: [`CUSTOMIZE.md`](../../CUSTOMIZE.md).

## Port 1: Services

The primary port. An EPS package is deployed into this port via `epc serve`. EPC reads
the package's `[service]` block in `eps.toml` to know how to run it.

This port has no upper limit — deploy as many services as your hardware can run.

## Port 2: Supervisor

The process supervision strategy. Default is EPC's built-in in-process loop.
Declared in the `[ports]` section of EPC's own `eps.toml`:

```toml
[ports]
supervisor = "builtin"   # default — in-process tokio supervisor
```

> **Note:** `launchd` and `systemd` supervisor backends are not yet implemented.
> For macOS login auto-start, use `epc install-startup` instead — it installs a
> LaunchAgent that runs `epc startup` at login. See [Auto-Start on Login](../users/startup.md).

## Port 3: Dashboard

EPC ships without a web UI. This port is intentionally empty. To fill it, deploy a
dashboard EPS — a web app that reads `~/.epc/services.toml` and renders a link page.

The contract for a dashboard EPS:
- Reads `~/.epc/services.toml` (or accepts it as a config path)
- Serves a web page listing services and their Tailscale URLs
- Is itself a service, deployed via `epc serve <dashboard-eps>`
