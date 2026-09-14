# Phase 03: Restart/offline resilience and observability

## Phase metadata

- Status: complete for generic runtime foundation
- Depends on: Phase 01, Phase 02
- Target: worker lifecycle, local state, logs, telemetry

## Outcome

The runtime can process work continuously and safely through reconnects, reports structured progress and metrics, and retains bounded local evidence for diagnosis.

## Tasks

- [x] P03-T01 Separate protocol I/O from execution workers with bounded concurrency, backpressure, leases, and graceful shutdown.
- [x] P03-T02 Add structured JSON logs, correlation IDs, metrics, traces, health/readiness details, and secret redaction.
- [x] P03-T03 Add local retention/cleanup, artifact manifests, result replay after reconnect, automatic deletion of detailed logs after 3 days, and a hard 4-day maximum.

## Validation milestones

- `V03-01`: failure injection covers worker crash, network loss, duplicate delivery, provider timeout, and late result.
- `V03-02`: soak test proves bounded disk/memory growth and stable heartbeats.
- `V03-03`: telemetry review confirms a single run can be traced from control-plane command to local side effect.

## Parallelization

- P03-T02 may proceed independently after event/correlation fields are frozen.

## Risks and mitigations

- Log retention can expose personal email/content: redact by default and make sensitive payload capture opt-in with explicit retention.

## Completion criteria

- [x] Runtime recovers from control-plane and process failures without duplicate successful side effects.
- [x] Operators can diagnose a failed run from the control plane and bounded runtime evidence.
- [x] Resource limits and cleanup are verified on the target LXC.
