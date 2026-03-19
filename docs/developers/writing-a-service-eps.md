# Writing a Service-Compatible EPS

An EPS is deployable via EPC if its `eps.toml` includes a `[service]` block.

## Minimum required

```toml
[service]
enabled = true
start   = "./run.sh serve"   # command EPC runs to start the service
port    = 8080               # default port (EPC may override to avoid conflicts)
```

## Full schema

```toml
[service]
enabled      = true
start        = "./run.sh serve"
port         = 8080
health_check = "GET /health"     # optional; used by `epc ps` to show running vs degraded
startup      = true              # optional; set false to exclude from `epc startup` auto-restart
```

## What EPC does with it

1. Install the package via EPM: `epm install <name>`
2. Deploy it: `epc deploy <name>`
3. EPC reads `[service]` from the installed `eps.toml`
4. It checks the port isn't already in use
5. It sets the `PORT` environment variable and runs `start` via `bash -c` in a new process group
6. It records the pid, port, and project dir in `~/.epc/services.toml`

## Port via environment variable

Your service should read its port from the `PORT` environment variable, not hard-code it.
EPC always sets `PORT` before starting your process, even if it matches your declared default.

```bash
#!/bin/bash
# run.sh
PORT=${PORT:-8080}
./my-server --port "$PORT"
```

## Health checks

If `health_check` is set, EPC polls the endpoint after startup and only marks the service
`running` once it responds 200. This gives `epc ps` an accurate status rather than
reporting running immediately after spawn.
