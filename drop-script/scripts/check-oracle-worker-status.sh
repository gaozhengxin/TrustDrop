#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ENV_FILE="$ROOT_DIR/drop-script/.env"
DEFAULT_URL="https://trustdrop-oracle-worker.zhengxingao.workers.dev"

if [ ! -f "$ENV_FILE" ]; then
  echo "drop-script/.env not found" >&2
  exit 1
fi

TOKEN="$(grep '^ORACLE_WORKER_TOKEN=' "$ENV_FILE" | tail -1 | cut -d= -f2-)"
WORKER_URL="$(grep '^ORACLE_WORKER_URL=' "$ENV_FILE" | tail -1 | cut -d= -f2-)"
STATUS_URL="$(grep '^ORACLE_WORKER_STATUS_URL=' "$ENV_FILE" | tail -1 | cut -d= -f2-)"

WORKER_URL="${WORKER_URL:-$DEFAULT_URL}"
STATUS_URL="${STATUS_URL:-${WORKER_URL%/}/status}"

if [ -z "$TOKEN" ]; then
  echo "ORACLE_WORKER_TOKEN is missing in drop-script/.env" >&2
  exit 1
fi

curl -sS -H "Authorization: Bearer $TOKEN" "$STATUS_URL"
echo
