# ADR-0004: EPC State File — `~/.epc/services.toml`

- **Date:** 2026-02-28
- **Status:** Accepted

## Context

EPC spawns processes and needs to track them across CLI invocations. `epc deploy`
starts a service and exits — the next time the user runs `epc ps` or `epc stop`, EPC
must know what processes it launched, their PIDs, ports, and where their logs are.

This tracking state must be:
- **Persistent** across CLI invocations (in-memory state is insufficient)
- **Human-readable** so users can inspect and manually edit it
- **Simple** — EPC is not a database; it doesn't need transactions or schemas

The closest analogy is `docker-compose.yml` (what services exist and how to reach them),
but the EPC state file describes **currently running** services, not the desired configuration.

| Container concept | EPC equivalent |
|---|---|
| `docker-compose.yml` | `~/.epc/services.toml` |
| `docker ps` | `epc ps` |
| Container ID | PID |
| Container stop | `epc stop <name>` |

## Decision

EPC stores its runtime state in `~/.epc/services.toml` — a single TOML file in the
user's home directory.

### Schema

```toml
[services.simple_todo]
dir      = "/Users/nick/Documents/personal-projects/simple_todo"
port     = 8765
pid      = 12345
started  = "2026-02-28T10:00:00Z"
log_file = "/Users/nick/.epc/logs/simple_todo.log"
```

Each key under `[services]` is the package name (from `eps.toml [package].name`). Fields:

| Field | Description |
|---|---|
| `dir` | Absolute path to the package directory — EPC uses this for `epc logs` |
| `port` | The declared port from `eps.toml [service].port` |
| `pid` | OS process ID of the running service |
| `started` | ISO 8601 timestamp of when EPC spawned the process |
| `log_file` | Absolute path to the log file (under `~/.epc/logs/`) |

### Location: `~/.epc/`

```
~/.epc/
  services.toml   ← the state file
  logs/
    simple_todo.log
    tech_talker.log
```

`~/.epc/` is created automatically by `epc deploy` on first use. The user owns this
directory and can inspect or back it up freely.

### Stale entry handling

A PID recorded in `services.toml` may no longer be alive if the process crashed or the
machine was rebooted. EPC detects stale entries lazily:

- `epc ps` checks `kill -0 <pid>` for each entry and reports status as `stopped`
  for dead processes (the entry is not auto-removed)
- `epc deploy` checks for a stale entry with the same name and removes it before
  re-registering the new process

EPC does not reap or auto-restart crashed services (see ADR-0005).

### Why TOML

TOML was chosen for the same reasons as `eps.toml` (EPM ADR-0003): it is human-readable,
supports inline comments, and has excellent Rust serde support via the `toml` crate.
The file is intended to be inspectable with any text editor.

The state file is a living document — EPC writes it on deploy/stop, but users can edit
it directly if they need to adjust a port, rename an entry, or clean up stale records.

### Why not SQLite

SQLite would support queries, transactions, and schema evolution. It is not needed here:
- The number of concurrently deployed services on a personal machine is small (< 100)
- No concurrent writers (EPC is a CLI, not a daemon)
- Human inspectability is more valuable than query performance

## Consequences

**Positive:**
- The state file is a TOML file the user owns and can read with any editor
- No database process to manage; no schema migrations
- `epc ps` and `epc stop` work correctly across reboots (modulo stale PID detection)
- Easy to back up, version-control, or sync across machines

**Negative:**
- No automatic cleanup of stale entries — `stopped` services linger until manually
  removed or until a redeploy of the same package name
- No locking — concurrent `epc deploy` calls could corrupt the file (acceptable for
  a single-user personal tool)
- Schema evolution requires careful backwards compatibility (adding optional fields is safe;
  renaming or removing fields is not)
