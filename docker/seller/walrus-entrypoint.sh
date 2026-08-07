#!/usr/bin/env bash
set -euo pipefail

log() {
  printf '[trustdrop-walrus] %s
' "$*" >&2
}

: "${WALRUS_HOME:=/home/justin/walrus}"
: "${WALRUS_CONFIG:=${WALRUS_HOME}/client_config.yaml}"
: "${WALRUS_WALLET:=/home/justin/.sui/sui_config/client.yaml}"
: "${WALRUS_SUB_WALLETS_DIR:=/home/justin/.sui/sui_config}"
: "${WALRUS_BIND_ADDRESS:=0.0.0.0:31415}"
: "${WALRUS_N_CLIENTS:=1}"

if [ ! -s "$WALRUS_CONFIG" ]; then
  log "missing WALRUS_CONFIG: $WALRUS_CONFIG"
  exit 20
fi
if [ ! -s "$WALRUS_WALLET" ]; then
  log "missing WALRUS_WALLET: $WALRUS_WALLET"
  exit 21
fi
if [ ! -d "$WALRUS_SUB_WALLETS_DIR" ]; then
  log "missing WALRUS_SUB_WALLETS_DIR: $WALRUS_SUB_WALLETS_DIR"
  exit 22
fi

log "walrus version: $(walrus --version)"
log "starting walrus publisher at http://${WALRUS_BIND_ADDRESS} using wallet ${WALRUS_WALLET}"
exec walrus daemon   --config "$WALRUS_CONFIG"   --wallet "$WALRUS_WALLET"   --bind-address "$WALRUS_BIND_ADDRESS"   --max-body-size 1048576000   --sub-wallets-dir "$WALRUS_SUB_WALLETS_DIR"   --n-clients "$WALRUS_N_CLIENTS"   --publisher-max-concurrent-requests "$WALRUS_N_CLIENTS"
