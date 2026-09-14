# Phase 02: Hermes adapter and safe execution engine

## Phase metadata

- Status: in progress — core Hermes runner implemented; operational hardening and automation fixtures remain pending
- Depends on: Phase 01
- Target: runner contract, Hermes command capability, executor, and first plan fixtures

## Outcome

The Rust runtime executes declarative plans produced by the control plane through a small allowlisted runner, with bounded subprocesses, streamed/redacted output, cancellation, and operation-level idempotency.

## Tasks

- [x] P02-T01 Define the typed Rust runner interface and implement configured-binary invocation, timeout, bounded output, failure handling, and basic process termination.
- [x] P02-T02 Implement the first approved hermes.run primitive; keep workflow orchestration and business rules out of this repository.
- [ ] P02-T03 Add generic execution fixtures for automation applications, preserving invocation, result, timeout, and duplicate-side-effect behavior without analyzing or porting individual automation logic here.

## Validation milestones

- `V02-01`: local process tests cover success, non-zero exit, timeout, bounded output, and redaction; cancellation, process groups, and escalation remain pending.
- `V02-02`: local fake Hermes tests verify the configured binary boundary; provider clients and deployment validation remain pending.
- `V02-03`: legacy gateway tests are ported and pass without using legacy filesystem/database paths.

## Parallelization

- P02-T02 adapter modules may be parallelized by non-overlapping capability ownership; P02-T03 integrates them sequentially.

## Risks and mitigations

- A command may complete externally before a local crash: assign operation IDs and reconciliation probes for Telegram, files, and provider mutations.

## Completion criteria

- [x] Hermes is the only supported agent and execution uses the runner capability boundary.
- [x] No remote payload can choose an arbitrary executable, cwd, or output path.
- [ ] Daily summary and standup behavior has parity evidence.
