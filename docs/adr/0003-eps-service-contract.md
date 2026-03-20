# ADR-0003: EPS Service Contract — the `[service]` Block and `serve.sh`

- **Date:** 2026-02-28
- **Status:** Accepted

## Context

EPC's job is to run EPS packages as persistent network services. To do that, EPC needs
to know three things about a package:

1. **Is it deployable?** Not every EPS is a network service. A CLI tool or a framework
   harness may have no concept of "listening on a port."
2. **How do I start it?** The entry point command, and whether it needs env vars.
3. **What port does it bind to?** So EPC can surface the right URL and detect conflicts.

These questions need a machine-readable answer, declared by the EPS author — not inferred
by EPC at deploy time.

The closest analogy in the container world:

| Container concept | EPC equivalent |
|---|---|
| `Dockerfile` (build + run instructions) | `[service]` block in `eps.toml` |
| `ENTRYPOINT` / `CMD` | `start` field |
| `EXPOSE <port>` | `port` field |
| `docker-compose.yml` (orchestration) | `~/.epc/services.toml` (see ADR-0004) |
| Docker daemon | EPC runtime |

Unlike Docker, EPC doesn't containerize anything. There are no images, no layers, no
namespaces. The analogy is conceptual only — the scope is much smaller.

## Decision

### The `[service]` block

An EPS that wants to be deployable via `epc serve` must include a `[service]` section
in its `eps.toml`:

```toml
[service]
enabled = true
start   = "./serve.sh"   # command EPC runs from the package directory
port    = 8765           # the port the process will bind to
```

**`enabled`** — explicit opt-in. A package with `enabled = false` (or no `[service]`
block at all) is not deployable via EPC. `epc serve` will fail immediately with a
clear message. This prevents accidental deployment of CLI-only packages.

**`start`** — the shell command EPC runs to start the service. It is executed via
`bash -c "<start>"` from the package directory. EPC passes `PORT=<n>` as an environment
variable; the `start` command may use `$PORT` or ignore it and bind to its declared port.

**`port`** — the port the service is expected to bind to. EPC does not yet enforce that
the process actually binds to this port (no health check), but uses it for URL
construction and future conflict detection.

### The `serve.sh` convention

For EPS packages that are not inherently network-aware (e.g. CLI tools), `serve.sh` is
the idiomatic way to implement the service mode:

```sh
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --quiet
exec ./target/release/my_pkg serve --port "${PORT:-8765}"
```

`serve.sh` is the ENTRYPOINT equivalent. It is responsible for:
- Building the binary if needed (first deploy may be slow; subsequent ones are no-ops)
- Launching the process that binds to the declared port
- Using `exec` so the PID EPC records is the real process, not a shell wrapper

The `serve` subcommand (or equivalent) in the binary is the port. The EPS author
decides what "serve" means — an HTTP API, a WebSocket server, a gRPC endpoint.

### What EPC does NOT provide

EPC is deliberately minimal. It does not:
- Build Docker images or manage containers
- Manage port allocation automatically (the declared port is used as-is)
- Health check the process after spawning
- Restart on crash (supervisor loop is a future extension — see ADR-0005)
- Provide TLS termination (Tailscale handles this via `tailscale cert` if desired)

## Example: `simple_todo`

`simple_todo` is a CLI todo list that ships a `serve` subcommand (axum HTTP API):

```toml
# simple_todo/eps.toml
[service]
enabled = true
start   = "./serve.sh"
port    = 8765
```

```sh
# simple_todo/serve.sh
cargo build --release --quiet
exec ./target/release/simple_todo serve --port "${PORT:-8765}"
```

```sh
epc serve simple_todo --local ./simple_todo
# → Deployed simple_todo → http://100.x.x.x:8765
```

The API is then reachable from any device in the tailnet at that URL.

## Consequences

**Positive:**
- Any EPS can become a network service by adding `[service]` and `serve.sh` — no EPC-specific
  framework or SDK required
- The `enabled` flag prevents accidental deploy of CLI-only packages
- The pattern is clear enough to document in one paragraph in `CUSTOMIZE.md`
- Composable: `serve.sh` can launch any process — a compiled binary, a Node server, a Python
  Flask app — EPC doesn't care

**Negative:**
- No port conflict detection yet — two packages declaring the same port will race
- No health check — EPC assumes the process is running if the PID is alive
- `serve.sh` must build the binary on first deploy; this is slow for large Rust projects
  (future: `epc serve` could run `cargo build --release` as a pre-step)
- The `start` command runs without a sandbox or resource limits — full trust model
