# Phase 02: Hermes adapter and safe execution engine

## Phase metadata

- Status: pending automation implementation
- Depends on: Phase 01
- Target: runner contract, Hermes command capability, executor, and first plan fixtures

## Outcome

The Rust runtime executes declarative plans produced by the control plane through a small allowlisted runner, with bounded subprocesses, streamed/redacted output, cancellation, and operation-level idempotency.

## Tasks

- [ ] P02-T01 Define the Rust runner/capability interface and implement Hermes CLI discovery, version/capability checks, timeout, signal handling, and process-group cleanup.
- [ ] P02-T02 Implement only the approved runner primitives required by control-plane plans; keep workflow orchestration and business rules out of this repository.
- [ ] P02-T03 Add generic execution fixtures for automation applications, preserving invocation, result, timeout, and duplicate-side-effect behavior without analyzing or porting individual automation logic here.

## Validation milestones

- `V02-01`: process tests cover success, non-zero exit, timeout, cancellation, SIGTERM/SIGKILL escalation, and redaction.
- `V02-02`: adapter contract tests use fake Hermes/TickTick/provider clients and verify allowlisted paths/commands.
- `V02-03`: legacy gateway tests are ported and pass without using legacy filesystem/database paths.

## Parallelization

- P02-T02 adapter modules may be parallelized by non-overlapping capability ownership; P02-T03 integrates them sequentially.

## Risks and mitigations

- A command may complete externally before a local crash: assign operation IDs and reconciliation probes for Telegram, files, and provider mutations.

## Completion criteria

- [ ] Hermes is the only supported agent but all execution uses the runner capability boundary.
- [ ] No remote payload can choose an arbitrary executable, cwd, or output path.
- [ ] Daily summary and standup behavior has parity evidence.
