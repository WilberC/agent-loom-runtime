# Phase 01: Secure runtime shell and protocol client

## Phase metadata

- Status: complete
- Depends on: None
- Target: `agent-loom-runtime`

## Outcome

The LXC service starts as a least-privilege daemon, authenticates to the control plane, registers capabilities, sends heartbeats, receives versioned commands, and persists enough state to resume after restart.

## Scope

- In: package bootstrap, settings, protocol client, local state schema, auth, health, graceful shutdown, systemd/container contract.
- Out: Hermes-specific side effects.

## Tasks

- [x] P01-T01 Bootstrap the Rust package, pinned toolchain, Cargo test/lint/build checks, and LXC service configuration without embedding machine-specific paths.
- [x] P01-T02 Implement protocol envelopes, capability registration, heartbeat, command acknowledgement, progress, result, and reconnect/resume behavior.
- [x] P01-T03 Implement local SQLite state with migrations, command dedupe, leases, and encrypted/indirect secret references.

## Validation milestones

- `V01-01`: cargo fmt, cargo clippy, cargo test, and cargo build pass through the chosen runner.
- `V01-02`: shared control-plane/runtime contract fixtures pass for valid, invalid, duplicate, replayed, and version-mismatched messages.
- `V01-03`: restart/offline integration test proves no command is silently lost.

## Parallelization

- P01-T02 and P01-T03 may proceed in parallel after the envelope and state ownership are agreed.

## Risks and mitigations

- Runtime cannot connect during control-plane outage: queue locally with bounded retention and expose explicit offline status.

## Completion criteria

- [x] The runtime connects outbound and reports capabilities.
- [x] Protocol authentication and replay protection are tested.
- [x] Restart behavior is deterministic and observable.
