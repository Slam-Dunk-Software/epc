# The State File

`~/.epc/services.toml` is EPC's live registry of running services. EPC reads and writes
it; you own it.

## Schema

```toml
[services.<name>]
dir      = "/path/to/project"   # absolute path to the project directory
port     = 8080                 # the allocated port
pid      = 12345                # OS process ID of the spawned process group leader
started  = "2026-02-27T10:00:00Z"
log_file = "/Users/you/.epc/logs/<name>.log"
```

Status (`running`, `stopped`, `degraded`) is computed live by `epc ps` — it is not
stored in the file. EPC checks whether the port is listening and, if the service
declares `health_check`, hits `/health` to distinguish `running` from `degraded`.

## Direct editing

The file is plain TOML. You can edit it directly to:
- Remove a stale entry after a hard crash
- Inspect the recorded pid or log path for a running service

EPC does not lock the file between commands, so avoid concurrent writes.
