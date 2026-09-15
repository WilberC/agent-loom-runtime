#!/usr/bin/env bash
set -euo pipefail

target="${1:-${AGENT_LOOM_RUNTIME_TARGET:-}}"
service="${AGENT_LOOM_RUNTIME_SERVICE:-agent-loom-runtime.service}"

if [[ -z "$target" ]]; then
  printf 'Usage: %s user@runtime-lxc\n' "$0" >&2
  printf 'Or set AGENT_LOOM_RUNTIME_TARGET.\n' >&2
  exit 2
fi

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
artifact="$repo_root/target/release/agent-loom"
remote_tmp="/tmp/agent-loom.new.$$"

printf '[DISCOVER] target=%s service=%s\n' "$target" "$service"
printf '[BUILD] running locked tests and release build\n'
if command -v mise >/dev/null 2>&1; then
  mise exec rust@1.85 -- cargo test --locked --manifest-path "$repo_root/Cargo.toml"
  mise exec rust@1.85 -- cargo build --release --locked --manifest-path "$repo_root/Cargo.toml"
else
  cargo test --locked --manifest-path "$repo_root/Cargo.toml"
  cargo build --release --locked --manifest-path "$repo_root/Cargo.toml"
fi

if [[ ! -x "$artifact" ]]; then
  printf '[ERROR] release artifact not found: %s\n' "$artifact" >&2
  exit 1
fi

printf '[UPLOAD] copying release artifact\n'
scp "$artifact" "$target:$remote_tmp"

printf '[INSTALL] replacing binary and restarting systemd\n'
ssh "$target" sh -s -- "$remote_tmp" "$service" <<'REMOTE'
set -eu
tmp=$1
service=$2
binary=/usr/local/bin/agent-loom
backup=/usr/local/bin/agent-loom.previous

if [ -x "$binary" ]; then
    sudo install -m 0755 "$binary" "$backup"
fi
sudo install -m 0755 "$tmp" "$binary"
rm -f "$tmp"

if sudo systemctl restart "$service" && sudo systemctl is-active --quiet "$service"; then
    printf '[OK] runtime updated and active\n'
    exit 0
fi

printf '[ROLLBACK] new binary failed to start\n' >&2
if [ -x "$backup" ]; then
    sudo install -m 0755 "$backup" "$binary"
    sudo systemctl restart "$service" || true
fi
exit 1
REMOTE

printf '[OK] deployment completed\n'
