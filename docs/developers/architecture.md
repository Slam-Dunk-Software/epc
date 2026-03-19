# Architecture

EPC is a single Rust binary (`epc`) built as a Cargo workspace with one member: `cli`.

```
epc/
  Cargo.toml           # workspace root
  cli/
    src/
      main.rs          # clap CLI dispatch
      eps.rs           # eps.toml parser (ServiceConfig)
      state.rs         # ServicesFile, is_port_listening, pids_on_port
      tailscale.rs     # tailscale status --json → Tailscale IP
      commands/
        deploy.rs        # resolve package dir + start daemon
        ps.rs            # read state file + health check + print table
        logs.rs          # tail process stdout/stderr
        stop.rs          # kill process group + port cleanup + update state
        restart.rs       # stop + wait for port quiet + respawn
        startup.rs       # restart all registered services (used at login)
        install_startup.rs  # install macOS LaunchAgent for epc startup
        self_update.rs   # update the epc binary via EPM
        audit.rs         # check for outdated installed packages
        observatory.rs   # node monitoring integration
```

## Data flow

```
epc deploy [<name>] [--local <path>]
  └─ resolve project dir         # --local path, cwd, or ~/.epm/packages/<name>/
  └─ read [service] from eps.toml  # learns start command + port
  └─ check port not already in use # via is_port_listening()
  └─ spawn process               # bash -c <start> in own process group (PGID = child PID)
  └─ write ~/.epc/services.toml  # records dir, pid, port, log_file, started

epc ps
  └─ read ~/.epc/services.toml   # loads registered service records
  └─ tailscale status --json     # resolves Tailscale IPv4 for URLs
  └─ per service: is_port_listening() → stopped / running / degraded
  └─ print table                 # NAME / PORT / PID / STATUS / URL

epc restart <name>
  └─ read ~/.epc/services.toml   # looks up dir, port, pid
  └─ kill -- -<pgid>             # kills entire process group (bash + children)
  └─ pids_on_port(port)          # also kills anything still on the port
  └─ poll is_port_listening()    # waits up to 5 s for port to go quiet
  └─ read [service] from eps.toml  # re-reads start command from recorded dir
  └─ spawn process               # fresh process group
  └─ write ~/.epc/services.toml  # records new pid + timestamp
```

## State file

`~/.epc/services.toml` is the single source of truth. See
[The State File](state-file.md) for the full schema.

## Tailscale integration

EPC shells out to `tailscale status --json` to discover the local node's hostname.
This requires Tailscale to be installed and the daemon to be running. See
[ADR-0002](../adr/0002-tailscale-as-networking-layer.md) for the rationale.
