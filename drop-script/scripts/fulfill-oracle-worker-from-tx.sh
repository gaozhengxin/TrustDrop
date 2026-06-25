#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ENV_FILE="$ROOT_DIR/drop-script/.env"
DEFAULT_URL="https://trustdrop-oracle-worker.zhengxingao.workers.dev"
CHAIN_ID="${CHAIN_ID:-421614}"

usage() {
  cat >&2 <<'EOF'
Usage:
  drop-script/scripts/fulfill-oracle-worker-from-tx.sh <tx-hash> [request-log-index]

Calls the centralized Oracle Worker /oracle/fulfill endpoint for an Arbitrum
Sepolia transaction that emitted OracleRequested from the configured OracleProxy.
The Worker token is read from drop-script/.env and is never printed.
EOF
}

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
  usage
  exit 1
fi

TX_HASH="$1"
REQUEST_LOG_INDEX="${2:-}"

if [ ! -f "$ENV_FILE" ]; then
  echo "drop-script/.env not found" >&2
  exit 1
fi

TOKEN="$(grep '^ORACLE_WORKER_TOKEN=' "$ENV_FILE" | tail -1 | cut -d= -f2-)"
WORKER_URL="$(grep '^ORACLE_WORKER_URL=' "$ENV_FILE" | tail -1 | cut -d= -f2-)"
WORKER_URL="${WORKER_URL:-$DEFAULT_URL}"
FULFILL_URL="${WORKER_URL%/}/oracle/fulfill"

if [ -z "$TOKEN" ]; then
  echo "ORACLE_WORKER_TOKEN is missing in drop-script/.env" >&2
  exit 1
fi

if ! [[ "$TX_HASH" =~ ^0x[0-9a-fA-F]{64}$ ]]; then
  echo "Invalid tx hash: $TX_HASH" >&2
  exit 1
fi

if [ -n "$REQUEST_LOG_INDEX" ] && ! [[ "$REQUEST_LOG_INDEX" =~ ^[0-9]+$ ]]; then
  echo "request-log-index must be a non-negative integer" >&2
  exit 1
fi

if [ -n "$REQUEST_LOG_INDEX" ]; then
  BODY="$(printf '{"chainId":%s,"txHash":"%s","requestLogIndex":%s}' "$CHAIN_ID" "$TX_HASH" "$REQUEST_LOG_INDEX")"
else
  BODY="$(printf '{"chainId":%s,"txHash":"%s"}' "$CHAIN_ID" "$TX_HASH")"
fi

curl -sS \
  -H "Authorization: Bearer $TOKEN" \
  -H "content-type: application/json" \
  --data "$BODY" \
  "$FULFILL_URL"
echo
