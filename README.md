# Agent Loom Runtime

A generic Rust runtime that initiates authenticated HTTP connections to an Agent Loom control plane. It contains no automation or Gmail business logic. Its first execution capability is the typed `hermes.run` runner, which invokes only the locally configured Hermes binary. Local delivered-event evidence is retained for at most four days while pending events are never pruned.

## Commands

```sh
mise exec rust@1.85 -- cargo run -- daemon
mise exec rust@1.85 -- cargo run -- tui
mise exec rust@1.85 -- cargo run -- doctor
```

The TUI is optional and reads local SQLite state; the daemon remains suitable for systemd.

## LXC updates

The runtime is deployed independently from the control plane on its dedicated
LXC. Build, test, upload, restart, and verify an update with one command from
this repository:

```sh
AGENT_LOOM_RUNTIME_TARGET=agent-loom@runtime-lxc \
  ./scripts/deploy-runtime.sh
```

The script uses the locked Rust toolchain through mise when available, keeps
the previous binary as `/usr/local/bin/agent-loom.previous`, and rolls back if
the systemd service does not become active. Configure passwordless sudo on the
LXC for installing `/usr/local/bin/agent-loom` and restarting
`agent-loom-runtime.service`. The runtime state and credentials are never
replaced by an update; keep `/var/lib/agent-loom` and `/etc/agent-loom`
intact.

## Configuration

Copy `.env.example` into a service-specific environment file. The runtime persists its runtime ID, issued secret, command deduplication records, and unsent lifecycle events in `AGENT_LOOM_STATE_PATH`. State is local only and must be permission-restricted by the service account. Enroll once with `AGENT_LOOM_ENROLLMENT_TOKEN`; afterward use `AGENT_LOOM_RUNTIME_SECRET_FILE` (recommended) or `AGENT_LOOM_RUNTIME_SECRET`.

The control-plane URL must not contain a path prefix; v1 paths are fixed below its root. Runtime credentials are never logged. `doctor` does not make network calls.

Hermes execution is configured with AGENT_LOOM_HERMES_BINARY, a timeout, and a maximum per-stream output size. Remote instructions can provide only a prompt; they cannot select an executable, working directory, environment, or output path. The runner invokes the configured binary as hermes run --prompt <prompt>.

The runtime also advertises `repo.sync` for repository automation applications. Its
initial safe operation is pull-only: repository URLs must use HTTPS, worktrees are
relative to `AGENT_LOOM_REPO_SYNC_ROOT` (default `repos`), and credentials are
provided ephemerally by the control plane. The credential is passed to Git through
environment-backed HTTP configuration and is never stored in the local command payload.

## Protocol v1 behavior

- Enrolls at `POST /api/v1/runtimes/register`, then uses `Authorization: Runtime <uuid>.<secret>`.
- Sends heartbeat envelopes and polls `/api/v1/runtime/commands` outbound.
- Stores commands before sending lifecycle events; duplicate command IDs are ignored.
- Queues ack, running progress, result, and error events locally and retries them in order after network failures. A `409` for an already-seen event is treated as delivered, so reconnects do not stall.
- Rejects non-v1 and unknown envelope fields while deserializing command collections.

See [`docs/protocol/runtime-v1.md`](docs/protocol/runtime-v1.md) for the copied contract and [`systemd/agent-loom-runtime.service`](systemd/agent-loom-runtime.service) for deployment hardening.

## Checks

```sh
mise exec rust@1.85 -- cargo fmt --check
mise exec rust@1.85 -- cargo clippy --all-targets -- -D warnings
mise exec rust@1.85 -- cargo test
mise exec rust@1.85 -- cargo build --release
```
