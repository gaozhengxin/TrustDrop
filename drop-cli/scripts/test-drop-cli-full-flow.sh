#!/usr/bin/env bash
set -euo pipefail

# TrustDrop drop-cli prototype full-flow test.
#
# This script intentionally uses only composite seller phases for the seller
# side:
#   1. drop-cli phase prepare
#   2. drop-cli phase publish
#   3. drop-cli phase complete-test-flow
#
# `complete-test-flow` is a prototype-only e2e phase. It calls the shared
# drop-script library implementation for buyer purchase, sale-bound VSS/VDD
# proofs, fulfill, centralized oracle trigger, oracle wait, and settle. Buyer
# purchase is still not a seller product command.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

if [ -z "${PROTOC:-}" ] && [ -x /tmp/protoc-25.3/bin/protoc ]; then
  export PROTOC=/tmp/protoc-25.3/bin/protoc
fi

TRUSTDROP_ENV="${TRUSTDROP_ENV:-drop-script/.env}"
DROP_CLI_ENV="${DROP_CLI_ENV:-$TRUSTDROP_ENV}"
RUN_DIR="${DROP_CLI_TEST_RUN_DIR:-/tmp/drop-cli-e2e-$(date +%Y%m%d-%H%M%S)}"
DROP_CLI_STATE_DIR="${DROP_CLI_STATE_DIR:-$RUN_DIR/state}"
ASSET_FILE=""
RECOVERED_FILE="KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile-recovered.mp4"

YES_WALRUS=0
YES_CHAIN=0
YES_PROVE=0
YES_ORACLE=0
YES_SETTLE=0

usage() {
  cat <<'USAGE'
Usage:
  drop-cli/scripts/test-drop-cli-full-flow.sh [options]

Options:
  --asset FILE     Use FILE as the e2e asset instead of generating a new file.
  --yes-walrus    Allow Walrus upload.
  --yes-chain     Allow Arbitrum Sepolia transactions.
  --yes-prove     Allow SP1 Prove Network proof requests.
  --yes-oracle    Allow centralized Oracle Worker report transaction.
  --yes-settle    Allow final settle transaction.
  -h, --help      Show this help.

The script exits before spending external resources unless all --yes-* gates
are supplied.
USAGE
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --asset)
      [ "$#" -ge 2 ] || { echo "missing value for --asset" >&2; exit 2; }
      ASSET_FILE="$2"
      shift 2
      ;;
    --yes-walrus) YES_WALRUS=1; shift ;;
    --yes-chain) YES_CHAIN=1; shift ;;
    --yes-prove) YES_PROVE=1; shift ;;
    --yes-oracle) YES_ORACLE=1; shift ;;
    --yes-settle) YES_SETTLE=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

log() {
  printf '\n[%s] %s\n' "$(date -Is)" "$*"
}

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

require_gate() {
  local name="$1"
  local value="$2"
  [ "$value" = 1 ] || fail "$name is required for full e2e"
}

require_env() {
  local key="$1"
  [ -n "${!key:-}" ] || fail "$key is missing in $TRUSTDROP_ENV"
}

load_env() {
  [ -f "$TRUSTDROP_ENV" ] || fail "TRUSTDROP_ENV not found: $TRUSTDROP_ENV"
  set -a
  # shellcheck disable=SC1090
  . "$TRUSTDROP_ENV"
  set +a
}

address_from_key() {
  cast wallet address --private-key "$1"
}

drop_cli() {
  if command -v nice >/dev/null 2>&1; then
    DROP_CLI_ENV="$DROP_CLI_ENV" nice -n "${DROP_CLI_NICE:-10}" target/debug/drop-cli "$@"
  else
    DROP_CLI_ENV="$DROP_CLI_ENV" target/debug/drop-cli "$@"
  fi
}

extract_last_value() {
  local key="$1"
  local file="$2"
  sed -n "s/^${key}: //p" "$file" | tail -1
}

ensure_drop_cli_env_file() {
  mkdir -p "$RUN_DIR" "$DROP_CLI_STATE_DIR"
  local test_env="$RUN_DIR/drop-cli.env"
  {
    printf '# drop-cli e2e generated settings\n'
    printf 'DROP_CLI_BASE_ENV=%s\n' "$TRUSTDROP_ENV"
    printf 'DROP_CLI_STATE_DIR=%s\n' "$DROP_CLI_STATE_DIR"
    printf 'ORACLE_MODE=centralized\n'
    printf 'TRUSTDROP_DEV_INSECURE_DEFAULT_KEYS=1\n'
  } > "$test_env"
  DROP_CLI_ENV="$test_env"
  export DROP_CLI_ENV DROP_CLI_STATE_DIR ORACLE_MODE
}

ensure_asset_file() {
  mkdir -p "$RUN_DIR"
  if [ -n "$ASSET_FILE" ]; then
    [ -f "$ASSET_FILE" ] || fail "asset file not found: $ASSET_FILE"
    return
  fi

  ASSET_FILE="$RUN_DIR/drop-cli-e2e-asset.bin"
  dd if=/dev/zero of="$ASSET_FILE" bs=1024 count=64 status=none
}

print_settings() {
  log "settings"
  printf 'TRUSTDROP_ENV=%s\n' "$TRUSTDROP_ENV"
  printf 'DROP_CLI_ENV=%s\n' "$DROP_CLI_ENV"
  printf 'DROP_CLI_STATE_DIR=%s\n' "$DROP_CLI_STATE_DIR"
  printf 'RUN_DIR=%s\n' "$RUN_DIR"
  printf 'ASSET_FILE=%s\n' "$ASSET_FILE"
  printf 'seller_address=%s\n' "$SELLER_ADDRESS"
  printf 'buyer_address=%s\n' "$BUYER_ADDRESS"
  printf 'HUB_ADDRESS=%s\n' "$HUB_ADDRESS"
  printf 'ORACLE_WORKER_URL=%s\n' "$ORACLE_WORKER_URL"
}

require_gate "--yes-walrus" "$YES_WALRUS"
require_gate "--yes-chain" "$YES_CHAIN"
require_gate "--yes-prove" "$YES_PROVE"
require_gate "--yes-oracle" "$YES_ORACLE"
require_gate "--yes-settle" "$YES_SETTLE"

load_env
require_env SELLER_KEY
require_env BUYER_KEY
require_env SP1_PRIVATE_KEY
require_env ORACLE_WORKER_URL
require_env ORACLE_WORKER_TOKEN
require_env HUB_ADDRESS
if [ -z "${ARBITRUM_SEPOLIA_RPC_URL:-}" ] && [ -z "${ARBITRUM_SEPOLIA_RPC:-}" ]; then
  fail "ARBITRUM_SEPOLIA_RPC_URL or ARBITRUM_SEPOLIA_RPC is missing in $TRUSTDROP_ENV"
fi

SELLER_ADDRESS="$(address_from_key "$SELLER_KEY")"
BUYER_ADDRESS="$(address_from_key "$BUYER_KEY")"

ensure_drop_cli_env_file
ensure_asset_file
print_settings

log "build drop-cli"
cargo build -p drop-cli

log "drop-cli init"
drop_cli init

log "drop-cli doctor"
drop_cli doctor

PREPARE_LOG="$RUN_DIR/prepare.log"
log "phase prepare"
if ! drop_cli phase prepare "$ASSET_FILE" >"$PREPARE_LOG" 2>&1; then
  cat "$PREPARE_LOG"
  fail "phase prepare failed"
fi
cat "$PREPARE_LOG"
SALE_ID="$(extract_last_value "saleId" "$PREPARE_LOG")"
[ -n "$SALE_ID" ] || fail "failed to parse saleId from $PREPARE_LOG"

PUBLISH_LOG="$RUN_DIR/publish.log"
log "phase publish"
if ! drop_cli phase publish "$SALE_ID" --yes >"$PUBLISH_LOG" 2>&1; then
  cat "$PUBLISH_LOG"
  fail "phase publish failed"
fi
cat "$PUBLISH_LOG"
ONCHAIN_SALE_ID="$(extract_last_value "phasePublishSaleId" "$PUBLISH_LOG")"
if [ -z "$ONCHAIN_SALE_ID" ]; then
  ONCHAIN_SALE_ID="$(extract_last_value "saleId" "$PUBLISH_LOG")"
fi
[ -n "$ONCHAIN_SALE_ID" ] || fail "failed to parse on-chain sale id from $PUBLISH_LOG"

COMPLETE_LOG="$RUN_DIR/complete-test-flow.log"
log "phase complete-test-flow"
if ! drop_cli phase complete-test-flow "$ONCHAIN_SALE_ID" --yes >"$COMPLETE_LOG" 2>&1; then
  cat "$COMPLETE_LOG"
  fail "phase complete-test-flow failed"
fi
cat "$COMPLETE_LOG"

log "verify recovered asset"
[ -f "$RECOVERED_FILE" ] || fail "recovered file not found: $RECOVERED_FILE"
ORIGINAL_SHA="$(sha256sum "$ASSET_FILE" | awk '{print $1}')"
RECOVERED_SHA="$(sha256sum "$RECOVERED_FILE" | awk '{print $1}')"
printf 'original_sha256=%s\n' "$ORIGINAL_SHA"
printf 'recovered_sha256=%s\n' "$RECOVERED_SHA"
[ "$ORIGINAL_SHA" = "$RECOVERED_SHA" ] || fail "recovered asset hash mismatch"

log "final status"
drop_cli status "$ONCHAIN_SALE_ID"

log "full flow completed"
printf 'sale_id=%s\n' "$ONCHAIN_SALE_ID"
printf 'run_dir=%s\n' "$RUN_DIR"
