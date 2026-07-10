#!/usr/bin/env bash
set -euo pipefail

PROJECT_NAME="${CLOUDFLARE_PAGES_PROJECT:-fair-file-marketplace}"
BRANCH_NAME="${CLOUDFLARE_PAGES_BRANCH:-dev}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WRANGLER_BIN="$ROOT_DIR/oracle-worker/node_modules/.bin/wrangler"

if [ ! -x "$WRANGLER_BIN" ]; then
  echo "Missing wrangler at $WRANGLER_BIN; run pnpm --dir oracle-worker install first." >&2
  exit 1
fi

pnpm --dir "$ROOT_DIR/app/gui" build
"$WRANGLER_BIN" pages deploy "$ROOT_DIR/app/gui/dist" \
  --project-name "$PROJECT_NAME" \
  --branch "$BRANCH_NAME" \
  --commit-dirty=true
