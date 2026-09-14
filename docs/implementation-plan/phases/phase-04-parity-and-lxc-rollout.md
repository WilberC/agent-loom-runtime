# Phase 04: Legacy parity and LXC cutover

## Phase metadata

- Status: pending automation implementation
- Depends on: Phase 03
- Target: LXC deployment and legacy replacement

## Outcome

The new runtime runs in dry-run mode, then takes ownership of the runner on the LXC after the legacy scheduler is stopped.

## Tasks

- [ ] P04-T01 Build a parity matrix from `tests/test_gateway.py` and legacy cron behavior, including idempotency, artifacts, auth, and failure semantics.
- [ ] P04-T02 Build, package, and install the Rust runtime binary on the LXC with service account, filesystem policy, secrets delivery, log cleanup, and health checks.
- [ ] P04-T03 Stop the legacy runtime, enable the new runtime, verify one plan at a time, and document a simple restore procedure; complex rollback orchestration is out of scope.

## Validation milestones

- `V04-01`: target-LXC smoke test validates Hermes/TickTick/Obsidian discovery and permissions without exposing secrets.
- `V04-02`: cutover and rollback are rehearsed with the control plane unavailable and with a runtime restart.
- `V04-03`: production canary meets the agreed reliability and no-duplicate-side-effect criteria before expansion.

## Risks and mitigations

- Both old and new schedulers run simultaneously: enforce a single owner per automation and verify it in the UI and runtime heartbeat.

## Completion criteria

- [ ] The generic runner contract is implemented and validated for the first automation applications.
- [ ] Only the new runtime executes production side effects.
- [ ] Legacy runtime is stopped and retained only as a read-only reference.
