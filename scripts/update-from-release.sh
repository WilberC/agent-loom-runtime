#!/usr/bin/env bash
set -euo pipefail

version="${1:-}"
config_file="${AGENT_LOOM_UPDATE_CONFIG:-/etc/agent-loom/update.env}"

if [[ -z "$version" ]]; then
  printf 'Usage: %s runtime-vX.Y.Z\n' "$0" >&2
  exit 2
fi

if [[ ! "$version" =~ ^runtime-v[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
  printf '[ERROR] invalid release version: %s\n' "$version" >&2
  exit 2
fi

if [[ ! -r "$config_file" ]]; then
  printf '[ERROR] configuration file not found: %s\n' "$config_file" >&2
  exit 1
fi

# shellcheck disable=SC1090
source "$config_file"
repository="${AGENT_LOOM_RUNTIME_REPOSITORY:-}"
service="${AGENT_LOOM_RUNTIME_SERVICE:-agent-loom-runtime.service}"
token="${AGENT_LOOM_GITHUB_TOKEN:-}"

if [[ ! "$repository" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]]; then
  printf '[ERROR] AGENT_LOOM_RUNTIME_REPOSITORY must be owner/repository\n' >&2
  exit 1
fi

if [[ -n "$token" ]]; then
  auth_header=(--header "Authorization: Bearer $token")
else
  auth_header=()
fi

asset="agent-loom-linux-amd64"
base_url="https://github.com/$repository/releases/download/$version"
work_dir=$(mktemp -d)
trap 'rm -rf "$work_dir"' EXIT

printf '[DOWNLOAD] %s %s\n' "$repository" "$version"
curl --fail --silent --show-error --location "${auth_header[@]}" \
  "$base_url/$asset" --output "$work_dir/$asset"
curl --fail --silent --show-error --location "${auth_header[@]}" \
  "$base_url/SHA256SUMS" --output "$work_dir/SHA256SUMS"

printf '[VERIFY] checking release checksum\n'
(cd "$work_dir" && grep -F "  $asset" SHA256SUMS | sha256sum --check --status -)

binary=/usr/local/bin/agent-loom
backup=/usr/local/bin/agent-loom.previous
tmp_binary="$binary.new.$$"

printf '[INSTALL] replacing binary and restarting systemd\n'
install -m 0755 "$work_dir/$asset" "$tmp_binary"
if [[ -x "$binary" ]]; then
  install -m 0755 "$binary" "$backup"
fi
mv -f "$tmp_binary" "$binary"

if systemctl restart "$service" && systemctl is-active --quiet "$service"; then
  printf '[OK] runtime updated to %s and active\n' "$version"
  exit 0
fi

printf '[ROLLBACK] release failed to start; restoring previous binary\n' >&2
if [[ -x "$backup" ]]; then
  install -m 0755 "$backup" "$binary"
  systemctl restart "$service" || true
fi
exit 1
