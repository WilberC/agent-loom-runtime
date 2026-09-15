# Agent Loom Runtime implementation plan

## Plan metadata

- Status: Hermes runner core implemented; automation adapters pending
- Scope: new runtime replacing `old-implementation/hermes-runtime`
- Target: `agent-loom-runtime`
- Last updated: 2026-09-15

## Source inventory

| Source | Contribution | Confidence |
| --- | --- | --- |
| `old-implementation/hermes-runtime/app` | Existing FastAPI gateway, registry, run store, cron entrypoints, and Hermes workflows | confirmed |
| `old-implementation/hermes-runtime/tests/test_gateway.py` | Existing idempotency, auth, artifact, and standup behavior to preserve | confirmed |
| User brief | Runtime lives only on Hermes LXC and talks directly to configured agents; Hermes is the first adapter | confirmed |
| New repository contents | No implementation or established stack yet | confirmed |

## Naming

- Repository: `agent-loom-runtime`.
- Runtime/product: Agent Loom Runtime.
- Binary: `agent-loom`.
- Daemon: `agent-loom daemon`.
- TUI: `agent-loom tui`.
- Diagnostics: `agent-loom doctor`.
- systemd unit: `agent-loom-runtime.service`.

Hermes is the first supported agent/adapter, not the name of the runtime itself.

## Objectives

- Provide a resilient local runtime that can execute Hermes automations on the LXC without exposing local tools or paths to the control plane.
- Receive declarative execution plans through the versioned control-plane protocol, report progress/results, and survive restarts/offline periods.
- Execute approved runner instructions for automation applications against Hermes initially; keep the runner contract extensible without moving automation business logic into the runtime.
- Make every execution timeout-aware, retry-aware, observable, and idempotent where the side effect requires it.

## Non-goals

- Hosting the operator UI or admin.
- Direct access from the control plane to the LXC filesystem or SQLite database.
- Supporting arbitrary shell commands supplied by an untrusted remote caller.

## Current-state summary

The predecessor is a synchronous FastAPI app with a bearer token, SQLite run records, a registry containing `daily-summary` and `daily-work-standup`, direct subprocess calls, Markdown output, cron entrypoints, and a direct SQLite write into the old Django project. It is a good behavior reference but not a safe general-purpose execution runner.

## Proposed approach

Use Rust for the runtime daemon and the separate Ratatui TUI. Build a single lightweight runtime binary with Tokio, an async outbound long-poll client, local SQLite state, with the control-plane persistence choice left open between SQLite and PostgreSQL, typed runner capabilities, bounded subprocess execution, and a least-privilege service account. The control plane owns workflow logic and sends declarative execution plans; the runtime only validates, runs, and reports them. The runtime should pull work rather than require an inbound public port. Add other agents and automation applications later through the same runner contract.

## Phase map

| Phase | Outcome | Depends on | Status |
| --- | --- | --- | --- |
| [Phase 01](phases/phase-01-runtime-foundation.md) | Secure runtime shell and protocol client | None | complete |
| [Phase 02](phases/phase-02-hermes-adapter-and-execution.md) | Hermes adapter and safe execution engine | Phase 01 | complete for generic runner foundation; automation adapters pending |
| [Phase 03](phases/phase-03-local-durability-and-telemetry.md) | Restart/offline resilience and observability | Phase 01, Phase 02 | complete for local foundation |
| [Phase 04](phases/phase-04-parity-and-lxc-rollout.md) | Legacy parity and LXC cutover | Phase 03 | planned |

## Cross-cutting risks and decisions

- **Protocol transport decided for MVP**: outbound authenticated long-poll; keep a transport interface so WebSocket can be added for live/high-volume installations.
- **TUI**: provide a local operator TUI as a separate client over a Unix socket; the runtime daemon must remain headless and systemd-managed.
- **Execution policy**: no arbitrary remote shell; use a signed allowlisted runner catalog, fixed working directories, explicit environment references, resource limits, and an unprivileged user.
- **Side effects**: persist operation-level idempotency, not only run-level idempotency; a crash between Telegram send and result persistence must be reconciled.
- **Artifacts**: write through an approved Obsidian adapter/path policy; never accept arbitrary paths from the control plane.
- **Future agents**: define runner capabilities around a stable execution contract, not vendor-specific routes.

## Execution contract

- Execute phases in dependency order.
- Mark checkboxes only after implementation and validation exist.
- Share protocol fixtures with `agent-loom-control-plane`.
- Run all commands through the eventual project-native runner once the empty repository is bootstrapped.
- Preserve the old implementation as a read-only parity reference until the generic platform milestone and contract tests are complete.
