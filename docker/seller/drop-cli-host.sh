#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"
DOCKER_BIN="${DOCKER_BIN:-}"
if [ -z "$DOCKER_BIN" ]; then
  if command -v docker >/dev/null 2>&1; then
    DOCKER_BIN="$(command -v docker)"
  elif [ -x /Applications/Docker.app/Contents/Resources/bin/docker ]; then
    DOCKER_BIN=/Applications/Docker.app/Contents/Resources/bin/docker
  else
    echo "docker CLI not found; set DOCKER_BIN" >&2
    exit 3
  fi
fi

usage() {
  cat <<'USAGE'
Usage:
  docker/seller/drop-cli-host.sh <drop-cli args...>

Examples:
  docker/seller/drop-cli-host.sh doctor
  docker/seller/drop-cli-host.sh sale list
  docker/seller/drop-cli-host.sh phase prepare ~/Desktop/demo.mp4
  docker/seller/drop-cli-host.sh asset prepare /Users/niuniu/Desktop/demo.mp4

For file-taking commands, the script accepts host paths and mounts the file's
parent directory read-only into a one-off seller-daemon container.
USAGE
}

if [ "$#" -eq 0 ] || [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  usage
  exit 0
fi

expand_path() {
  local value="$1"
  case "$value" in
    "~") printf '%s\n' "$HOME" ;;
    "~/"*) printf '%s/%s\n' "$HOME" "${value#~/}" ;;
    /*) printf '%s\n' "$value" ;;
    *) printf '%s/%s\n' "$PWD" "$value" ;;
  esac
}

run_exec() {
  local exec_flags=(-i)
  if [ -t 0 ] && [ -t 1 ]; then
    exec_flags=(-it)
  fi
  "$DOCKER_BIN" exec "${exec_flags[@]}" trustdrop-seller-daemon target/debug/drop-cli "$@"
}

run_oneoff_with_file() {
  local host_file="$1"
  shift
  host_file="$(expand_path "$host_file")"
  if [ ! -f "$host_file" ]; then
    echo "host file not found: $host_file" >&2
    exit 2
  fi
  local host_dir base container_file
  host_dir="$(cd -- "$(dirname -- "$host_file")" && pwd)"
  base="$(basename -- "$host_file")"
  container_file="/host-input/$base"
  cd "$SCRIPT_DIR"
  "$DOCKER_BIN" compose --env-file /dev/null run --rm \
    --no-deps \
    --name "trustdrop-seller-cli-$(date +%s)-$$" \
    -v "$host_dir:/host-input:ro" \
    seller-daemon \
    bash -lc 'target/debug/drop-cli "$@"' _ "$@" "$container_file"
}

case "${1:-} ${2:-}" in
  "asset prepare")
    if [ "$#" -ne 3 ]; then echo "usage: $0 asset prepare <host-file>" >&2; exit 2; fi
    run_oneoff_with_file "$3" asset prepare
    ;;
  "phase prepare")
    if [ "$#" -ne 3 ]; then echo "usage: $0 phase prepare <host-file>" >&2; exit 2; fi
    run_oneoff_with_file "$3" phase prepare
    ;;
  *)
    run_exec "$@"
    ;;
esac
