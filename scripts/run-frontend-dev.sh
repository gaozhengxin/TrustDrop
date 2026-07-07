#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
pnpm --dir app/gui dev --host 0.0.0.0
