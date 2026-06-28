#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ENV_FILE="$ROOT_DIR/drop-script/.env"
DEFAULT_URL="https://trustdrop-oracle-worker.zhengxingao.workers.dev"

usage() {
  cat >&2 <<'EOF'
Usage:
  drop-script/scripts/check-oracle-worker-blob-status.sh --blob-id <walrus-blob-id>
  drop-script/scripts/check-oracle-worker-blob-status.sh --c-cipher <0x-hex-bytes>

Checks the centralized Oracle Worker /walrus/blob-status endpoint.
The Worker token is read from drop-script/.env and is never printed.
EOF
}

if [ "$#" -ne 2 ]; then
  usage
  exit 1
fi

MODE="$1"
VALUE="$2"

if [ ! -f "$ENV_FILE" ]; then
  echo "drop-script/.env not found" >&2
  exit 1
fi

TOKEN="$(grep '^ORACLE_WORKER_TOKEN=' "$ENV_FILE" | tail -1 | cut -d= -f2-)"
WORKER_URL="$(grep '^ORACLE_WORKER_URL=' "$ENV_FILE" | tail -1 | cut -d= -f2-)"
WORKER_URL="${WORKER_URL:-$DEFAULT_URL}"

if [ -z "$TOKEN" ]; then
  echo "ORACLE_WORKER_TOKEN is missing in drop-script/.env" >&2
  exit 1
fi

case "$MODE" in
  --blob-id)
    if ! [[ "$VALUE" =~ ^[A-Za-z0-9_-]+$ ]]; then
      echo "Invalid Walrus blob id: $VALUE" >&2
      exit 1
    fi
    QUERY="blobId=$VALUE"
    ;;
  --c-cipher)
    if ! [[ "$VALUE" =~ ^0x[0-9a-fA-F]+$ ]]; then
      echo "Invalid cCipher hex bytes: $VALUE" >&2
      exit 1
    fi
    QUERY="cCipher=$VALUE"
    ;;
  *)
    usage
    exit 1
    ;;
esac

curl -sS -H "Authorization: Bearer $TOKEN" \
  "${WORKER_URL%/}/walrus/blob-status?$QUERY"
echo
