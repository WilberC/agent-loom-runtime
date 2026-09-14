# Phase 02: Hermes adapter and safe execution engine

## Phase metadata

- Status: complete for the generic runner foundation — automation fixtures and legacy parity remain pending
- Depends on: Phase 01
- Target: runner contract, Hermes command capability, executor, and first plan fixtures

## Outcome

The Rust runtime executes declarative plans produced by the control plane through a small allowlisted runner, with bounded subprocesses, streamed/redacted output, cancellation, and operation-level idempotency.

## Tasks

- [x] P02-T01 Define the typed Rust runner interface and implement configured-binary invocation, timeout, bounded output, failure handling, and process-group termination.
- [x] P02-T02 Implement the first approved hermes.run primitive; keep workflow orchestration and business rules out of this repository.
- [x] P02-T03 Add generic execution fixtures for automation applications, preserving invocation, result, timeout, and duplicate-side-effect behavior without analyzing or porting individual automation logic here.

## Validation milestones

- `V02-01`: local process tests cover success, non-zero exit, timeout, bounded output, redaction, cancellation, and process-group cleanup. Unix termination sends `SIGTERM`, waits two seconds, and escalates to `SIGKILL`.
- `V02-02`: local fake Hermes tests verify the configured binary boundary; provider clients and deployment validation remain pending.
- `V02-04`: `daemon::process_command` is exercised with local SQLite state and a fake configured Hermes binary for success, non-zero failure, unsupported instructions, and cancellation; each case preserves `ack -> running progress -> result/error` ordering.
- `V02-05`: generic daemon fixtures cover invalid instructions and replayed command deduplication without starting a control-plane service.
- `V02-06`: disposable Docker integration verifies the generic success and failure lifecycle, replay deduplication, and recovery after a runtime outage against a real control-plane process.
- `V02-03`: legacy gateway tests are ported and pass without using legacy filesystem/database paths.

## Parallelization

- P02-T02 adapter modules may be parallelized by non-overlapping capability ownership; P02-T03 integrates them sequentially.

## Risks and mitigations

- A command may complete externally before a local crash: assign operation IDs and reconciliation probes for Telegram, files, and provider mutations.

## Completion criteria

- [x] Hermes is the only supported agent and execution uses the runner capability boundary.
- [x] No remote payload can choose an arbitrary executable, cwd, or output path.
- [x] Cancellation is exposed through a runner API and daemon shutdown cancels an active Hermes execution.
- [ ] Daily summary and standup behavior has parity evidence.
