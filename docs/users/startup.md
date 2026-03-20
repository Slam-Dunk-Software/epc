# Auto-Start on Login (macOS)

EPC can automatically restart all your registered services when you log in to your Mac.
This is done via a macOS LaunchAgent — a user-level background job that runs at login
without requiring root or `sudo`.

> **macOS only.** `epc install-startup` and `epc startup` work on macOS.
> Linux support (via systemd user units) is not yet implemented.

## Setup

Run once after deploying your services:

```bash
epc install-startup
```

This creates `~/Library/LaunchAgents/com.eps.epc-startup.plist` and loads it immediately.
From now on, every time you log in, `epc startup` runs automatically in the background.

## What happens at login

1. macOS starts the LaunchAgent after you log in.
2. `epc startup` waits up to 30 seconds for Tailscale to be connected.
3. For each service in `~/.epc/services.toml`:
   - If its port is already listening → skipped (already running).
   - If its project directory exists → deployed via `epc serve --local <dir>`.
   - If the directory is missing → warning printed, skipped.
4. A summary is written to `~/.epc/logs/startup.log`.

## Check startup logs

```bash
cat ~/.epc/logs/startup.log
```

Or tail in real time on the next login:

```bash
tail -f ~/.epc/logs/startup.log
```

## Run it manually

You can trigger the same startup sequence at any time:

```bash
epc startup
```

This is useful after a crash, a manual `epc stop`, or any time you want to bring
everything back up without remembering individual service names.

## Uninstall

```bash
launchctl unload ~/Library/LaunchAgents/com.eps.epc-startup.plist
rm ~/Library/LaunchAgents/com.eps.epc-startup.plist
```

## Opting a service out of auto-start

Add `startup = false` to the `[service]` block in any service's `eps.toml`:

```toml
[service]
enabled = true
startup = false   # won't be restarted by `epc startup`
start = "..."
port = 8080
```

The service can still be deployed and restarted manually — `startup = false` only
affects `epc startup` (and by extension the login LaunchAgent). The default is `true`,
so existing services are unaffected unless you explicitly opt out.

## How it relates to services.toml

`epc startup` reads `~/.epc/services.toml` — the same state file that `epc ps` and
`epc serve` use. Any service you have ever deployed is recorded there. If you
permanently decommission a service, `epc stop <name>` removes it from the file so
it won't be restarted on the next login.
