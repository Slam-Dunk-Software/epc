# ADR-0005: Process Lifecycle and Log Management

- **Date:** 2026-02-28
- **Status:** Accepted

## Context

`epc deploy` spawns a service process and exits. The service must continue running
after the CLI exits, stdout/stderr must be captured somewhere useful, and `epc stop`
must be able to terminate the process later.

This ADR documents:
1. How processes are spawned (spawn model, env vars, working directory)
2. How stdout/stderr are captured (log files)
3. How processes are stopped (signal model)
4. What EPC does not do (supervision, restart, health checks)

## Decision

### Spawning

`epc deploy` spawns the service via:

```rust
tokio::process::Command::new("bash")
    .arg("-c")
    .arg(&service_config.start)  // from eps.toml [service].start
    .current_dir(&pkg_dir)
    .env("PORT", port.to_string())
    .stdout(log_file)
    .stderr(log_file)
    .spawn()
```

Key decisions:

**`bash -c "<start>"`** — The `start` command is run through bash rather than executed
directly. This allows `serve.sh` to be a path (not just a binary name), supports shell
features like `exec`, and is consistent with how users would run the command manually.

**`current_dir = pkg_dir`** — The process starts with its working directory set to
the package root. `serve.sh` and relative paths in the start command resolve correctly
without any `cd` needed.

**`PORT=<n>` env var** — EPC passes the declared port as `PORT` so the service can read
it from the environment if it chooses. The service may also ignore `PORT` and use its
compiled-in default — EPC does not enforce consistency.

**Detached** — The child process is spawned and immediately handed off. `epc deploy`
records the PID and exits. There is no supervisor loop or long-running EPC process.
The service runs as an independent OS process.

### Log capture

All stdout and stderr from the service process are redirected to a single log file:

```
~/.epc/logs/<name>.log
```

The log file is created fresh on each `epc deploy` (overwriting any previous log for
that service name). `epc logs <name>` runs `tail -f` on the log file for live streaming.

**Why a single log file (not split stdout/stderr):** Services commonly interleave both
streams (e.g. axum logs to stderr, app output to stdout). A single file preserves
chronological order without merging complexity.

**Why `~/.epc/logs/` (not the package dir):** Log files are runtime artifacts, not source
artifacts. Keeping them in `~/.epc/` makes them easy to find, easy to clean up, and
avoids polluting the package directory (which may be a git repo).

### Stopping and restarting

**The port is the source of truth, not the cached PID.**

The PID recorded in `~/.epc/services.toml` at deploy time goes stale the moment anything
touches the process outside EPC (crash, manual restart, signal from another tool). Using
that PID for kill operations sends SIGTERM to the wrong process and leaves the real one
running.

`epc stop` and `epc restart` instead derive live PIDs from the port at call-time:

```rust
// state.rs
pub fn pids_on_port(port: u16) -> Vec<u32> {
    // lsof -t -i :PORT -sTCP:LISTEN
}
```

This returns every process currently listening on the port (any interface), which is
exactly what needs to be killed. The cached PID in services.toml is kept for display
in `epc ps` but is never used for signaling.

`epc restart` additionally waits for the port to go quiet (up to 5s) before spawning
the replacement, ensuring no duplicate instances start on the same port.

### Health checks

Services may declare a health endpoint in `eps.toml`:

```toml
[service]
health_check = "GET /health"
```

`epc ps` reads this field from the package's `eps.toml` at runtime and makes a GET
request to `http://<tailscale-ip>:<port>/health` with a 2s timeout. Status is reported
as `running` (200), `degraded` (non-200 or timeout), or `stopped` (port not listening).

Services are responsible for what their `/health` endpoint checks. A service with
downstream dependencies should probe those dependencies and return 503 if any are
unreachable — EPC only needs to see the top-level result.

### Port conflict detection

`epc deploy` checks whether the declared port is already listening before spawning,
regardless of whether the service is tracked in `services.toml`. This catches both
"already deployed via EPC" and "started manually outside EPC":

```rust
} else if ServicesFile::is_port_listening(svc.port) {
    bail!("port {} is already in use — is '{name}' running outside EPC?", svc.port);
}
```

### What EPC does NOT do

**No supervisor loop.** EPC does not watch the service process and restart it on crash.
The design decision is that supervision is an OS concern (launchd on macOS, systemd on
Linux). A future ADR may add opt-in supervisor integration.

**No resource limits.** The service process runs with the same permissions and resource
limits as the user's shell session. No cgroups, no ulimits.

## Consequences

**Positive:**
- Services survive `epc deploy` exiting — no long-lived EPC daemon required
- Log files in `~/.epc/logs/` are easy to find and inspect
- SIGTERM gives services a chance to shut down gracefully
- The spawn model is simple enough to test without process mocking

**Negative:**
- Services do not auto-restart on crash — the user must re-run `epc deploy`
- SIGTERM without SIGKILL followup means a misbehaving service may not actually stop
- Log files are overwritten on redeploy — no log rotation or history
