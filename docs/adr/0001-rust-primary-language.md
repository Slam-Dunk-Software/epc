# ADR-0001: Use Rust as the Primary Implementation Language

- **Date:** 2026-02-27
- **Status:** Accepted

## Context

EPC (Extremely Personal Cloud) is a local process supervisor and personal PaaS runtime for
EPS packages. It manages service lifecycles, port allocation, log streaming, and Tailscale
integration — all running on the user's own hardware. It is part of the same ecosystem as
EPM (the package manager), which is already implemented in Rust.

The implementation language choice has implications for correctness, performance, binary
distribution, and ecosystem consistency.

Alternatives considered:

| Language | Notes |
|----------|-------|
| Go       | Strong concurrency story, easy cross-compilation, but diverges from EPM's Rust codebase |
| Python   | Rapid prototyping, but requires a runtime; poor fit for a system-level daemon |
| Rust     | Memory-safe, single static binary, strong async story (tokio), consistent with EPM |

## Decision

Use Rust for all EPC components. The CLI (`epc`) and any future daemon component will be
written in Rust, following the same patterns established in the EPM codebase (clap for CLI
dispatch, tokio for async, anyhow for error handling).

Consistency with EPM is a first-class concern: users and contributors should be able to
move between the two codebases without context-switching on language or idioms.

## Consequences

**Positive:**
- Single static binary — easy to install (`cargo install epc`), no runtime dependency
- Memory safety eliminates a class of bugs relevant to long-running daemons
- Shared patterns and dependencies with EPM (clap, tokio, serde, anyhow)
- Strong async support for concurrent log tailing and process supervision

**Negative:**
- Slower iteration than scripting languages during early prototyping
- Compile times are longer than Go equivalents
- Rust expertise required for contributors
