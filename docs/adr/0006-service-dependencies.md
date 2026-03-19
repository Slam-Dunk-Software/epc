# ADR-0006: Service Dependencies — `depends_on` in `[service]`

- **Date:** 2026-03-01
- **Status:** Draft

## Context

As the EPS ecosystem grows, services increasingly depend on other services at runtime.
`morning_brief`, for example, is useless without both `simple_todo` (to read todos from)
and `txtme` (to deliver the SMS). Right now there is no way to express this. The
consequence is silent failure: `epc deploy morning_brief` succeeds even if neither
dependency is running, and the user only discovers the problem when the 9am brief
doesn't arrive.

This is the same problem Docker Compose's `depends_on` and systemd's `Requires=` solve
in their respective worlds. The EPC equivalent should be lighter than both — no
orchestration graphs, no health-check polling, no restart cascades. Just enough to
surface the dependency and let EPC warn (or act) before a broken deploy goes unnoticed.

There are two distinct dependency surfaces to consider:

| Layer | Question | Tool |
|---|---|---|
| **Install-time** | Is the required EPS installed on this machine? | EPM |
| **Runtime** | Is the required service currently running under EPC? | EPC |

Both are real needs. This ADR addresses the **runtime** layer (EPC's responsibility)
and proposes the `eps.toml` schema extension that EPM would also read for install-time
enforcement in a future ADR.

## Decision

### `eps.toml` schema extension

Add an optional `depends_on` field to the `[service]` block:

```toml
[service]
enabled    = true
start      = "./serve.sh"
port       = 5544
depends_on = ["txtme", "simple_todo"]
```

`depends_on` is a list of EPS package names — the same names used with `epc deploy`
and stored in `~/.epc/services.toml`. An absent or empty `depends_on` means no runtime
dependencies, which preserves full backwards compatibility.

### EPC behavior at deploy time

When `epc deploy <name>` is called for a package with `depends_on`, EPC checks
`~/.epc/services.toml` for each declared dependency:

1. **Present and `running`** → proceed normally.
2. **Present but `stopped`** → hard-block with an actionable message:
   ```
   error: dependency 'txtme' is installed but not running
     fix: epc restart txtme
   ```
3. **Not present** → hard-block with an actionable message:
   ```
   error: dependency 'txtme' is not deployed
     fix: epc deploy txtme
   ```

EPC does **not** auto-deploy or auto-restart missing dependencies. Side effects on
other services should only happen with explicit user consent (same principle as
ADR-0013's stance on system dependencies).

### `epc ps` — surfacing the dependency graph

`epc ps` should indicate when a running service's dependencies are unhealthy:

```
NAME            PORT   STATUS     URL
morning_brief   5544   degraded   http://100.x.x.x:5544   (txtme: stopped)
txtme           5543   stopped    http://100.x.x.x:5543
simple_todo     8765   running    http://100.x.x.x:8765
```

`degraded` = the service process itself is alive, but at least one declared dependency
is not running. This surfaces the problem without stopping the dependent service — it
may still be healthy in a reduced capacity, or the dependency may have only briefly
restarted.

### `epc stop` — cascading awareness (not cascading action)

When `epc stop <name>` would leave a dependent service in a degraded state, EPC warns
before stopping:

```
warning: stopping 'txtme' will degrade 1 dependent service:
  morning_brief (depends on txtme)
Stop anyway? [y/N]
```

EPC does not auto-stop dependents. The warning is informational.

### State file changes

The `~/.epc/services.toml` record for a deployed service gains a `depends_on` field
(copied from `eps.toml` at deploy time):

```toml
[services.morning_brief]
dir        = "/Users/nick/Documents/personal-projects/morning_brief"
port       = 5544
pid        = 12345
started    = "2026-03-01T09:00:00Z"
log_file   = "/Users/nick/.epc/logs/morning_brief.log"
depends_on = ["txtme", "simple_todo"]
```

This allows EPC to check dependency health at any time without re-reading source `eps.toml`
files, and lets `epc ps` compute `degraded` status purely from the state file.

## What this ADR defers

**Install-time enforcement (EPM layer):** EPM could read `depends_on` and check (or
offer to install) missing packages before the install completes. This is valuable but
belongs in an EPM-focused ADR — the registry schema, `epm install` behavior, and
dependency resolution are EPM concerns. The `eps.toml` field proposed here is
intentionally compatible with a future EPM extension.

**Version constraints:** `depends_on = ["txtme>=1.0.0"]` is a natural extension but
adds resolver complexity. Deferred until there is a concrete case where version
mismatches have caused real breakage.

**Optional / soft dependencies:** Some services degrade gracefully without a dependency
rather than failing entirely. A future `depends_on_soft = [...]` field could express
this. Deferred — the simple case covers the current need.

**Circular dependency detection:** EPC should detect and reject circular `depends_on`
graphs at deploy time. Straightforward to implement (DFS over the state file), deferred
from this ADR to keep scope narrow.

## Consequences

**Positive:**
- Silent broken deploys become loud, actionable errors
- `epc ps` gives an honest picture of ecosystem health, not just process health
- The `eps.toml` field is forwards-compatible with EPM install-time enforcement
- No orchestration complexity — EPC checks and warns, the user acts

**Negative:**
- `epc deploy` gains a new failure mode; users with no dependencies are unaffected,
  but users who declare dependencies must ensure ordering
- `epc stop` warnings add friction to teardown; could annoy during development
- `degraded` status requires EPC to understand the dep graph, increasing `epc ps`
  complexity slightly
- Does not help if a dependency crashes *after* deploy — runtime health is not
  continuously monitored (see ADR-0005 on process lifecycle)
