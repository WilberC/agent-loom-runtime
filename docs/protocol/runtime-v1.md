# Runtime protocol v1

All runtime JSON is a **v1 envelope**: `protocol_version` (`"v1"`), UUID `message_id`, and RFC 3339 `sent_at`. Unknown fields are rejected in v1 envelopes. The canonical machine-readable schemas are maintained in the Agent Loom repository at `protocol/schemas.py`; the runtime must consume a copied/generated equivalent and use its fixture tests.

## Authentication and enrollment

`POST /api/v1/runtimes/register` accepts a `registration` envelope, `X-Enrollment-Token`, and `Idempotency-Key`. Its response exposes `runtime_secret` exactly once. The runtime subsequently authenticates as `Authorization: Runtime <runtime-uuid>.<runtime-secret>`. The database stores only a SHA-256 digest; enrollment secrets and issued credentials must not be logged or committed.

A Django user/session is the separate browser/admin boundary. No runtime credential grants browser permissions, and the initial API has no user-facing automation operation.

## Endpoints and lifecycle

| Direction | Endpoint | Envelope | Outcome |
| --- | --- | --- | --- |
| runtime → control plane | `POST /api/v1/runtimes/register` | registration | stable runtime identity |
| runtime → control plane | `POST /api/v1/runtime/heartbeat` | heartbeat | last-seen signal |
| runtime ← control plane | `GET /api/v1/runtime/commands` | command collection | leases one declarative command when available |
| runtime → control plane | `POST /api/v1/runtime/events/ack` | ack | starts a command lifecycle |
| runtime → control plane | `POST /api/v1/runtime/events/progress` | progress | requires ack/progress |
| runtime → control plane | `POST /api/v1/runtime/events/result` | result | terminal; requires ack/progress |
| runtime → control plane | `POST /api/v1/runtime/events/error` | error | terminal; requires ack/progress |

`message_id` is unique per runtime. Duplicate message IDs, terminal follow-ups, and out-of-order events return `409`; malformed or wrong-version JSON returns `400`; missing/invalid runtime identity returns `401`. This deliberately records protocol receipts only, not automation execution logic.

`GET /healthz` is process liveness; `GET /readyz` additionally checks database availability. Both return an `X-Correlation-ID`, reusing one supplied by the caller or generating one.

## Compatibility

Future versions must add fields/endpoints or negotiate capabilities; do not repurpose v1 fields. WebSocket transport may later carry the same envelopes, while v1 uses outbound runtime polling only.
The Rust test suite consumes the deterministic fixture copied to [`tests/fixtures/runtime-v1.json`](../../tests/fixtures/runtime-v1.json). Keep this copy synchronized with the control-plane fixture at `agent-loom/docs/protocol/fixtures/runtime-v1.json`; no network or running service is required for contract validation.

## Hermes runner instruction

The runtime currently advertises the `hermes.run` and `repo.sync` capabilities. A command for `hermes.run` has exactly this instruction shape:

    {"kind":"hermes.run","prompt":"Produce the requested structured response"}

The runtime rejects unknown fields, unsupported kinds, empty prompts, and prompts larger than 128 KiB. It executes the configured local binary as hermes run --prompt <prompt>; the command cannot choose an executable, working directory, environment, or output path. Each accepted command queues ack, progress with {"percent":0,"status":"running"}, and then exactly one result or error event. Output is bounded and common bearer, token, secret, password, and API-key formats are redacted before being included in results or errors.

### Repository synchronization instruction

`repo.sync` supports pull, push, and pull-push operations and has this shape:

```json
{
  "kind": "repo.sync",
  "repository_url": "https://github.com/example/repository.git",
  "operation": "pull",
  "reference": "main",
  "worktree": "example-repository",
  "credential": {"username": "x-access-token", "token": "injected-ephemerally"}
}
```

The control plane stores only a credential reference and expands it when leasing
the command. The runtime stores a digest of the received instruction for
deduplication, not the instruction itself. Worktrees are relative to the runtime
sync root; credentials are supplied to Git through environment-backed config and
never embedded in the repository URL. Push operations require an existing
worktree and update `origin:<reference>` from the worktree's current `HEAD`; they
never create commits or push uncommitted changes.
