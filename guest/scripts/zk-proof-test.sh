#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PROTOC_BIN="${PROTOC:-/tmp/protoc-25.3/bin/protoc}"
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
SP1_GATEWAY_GROTH16="${SP1_GATEWAY_GROTH16:-0x397A5f7f3dBd538f23DE225B51f532c34448dA9B}"
DEFAULT_RPC_URL="https://sepolia-rollup.arbitrum.io/rpc"

usage() {
  cat <<'USAGE'
Usage:
  guest/scripts/zk-proof-test.sh <vss|vdd> <execute|prove|local-contract|preflight|all>

Stages:
  execute         Build and run the local SP1 execute path. No proof request.
  prove           Build the EVM proof binary, then request Groth16 proof from SP1 Prove Network.
  local-contract  Run local Foundry wrapper tests with mocked SP1 gateway calldata.
  preflight       eth_call official Arbitrum Sepolia SP1 Groth16 gateway using current fixture.
  all             Run execute, prove, local-contract, preflight in that order.

Environment:
  PROTOC=/tmp/protoc-25.3/bin/protoc
  CARGO_BUILD_JOBS=1
  TRUSTDROP_ENV=drop-script/.env
  ARBITRUM_SEPOLIA_RPC or ARBITRUM_SEPOLIA_RPC_URL, default https://sepolia-rollup.arbitrum.io/rpc
  VDD_RSLHVE_DATA_SIZE=65536 for VDD prove/execute input size.

Notes:
  - prove uses SP1 Prove Network and may consume PROVE credit on successful proving.
  - prove reads SP1_PRIVATE_KEY from TRUSTDROP_ENV and maps it to NETWORK_PRIVATE_KEY for SP1 SDK.
  - secrets are never printed by this script.
USAGE
}

die() {
  echo "error: $*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

load_network_env() {
  local env_file="${TRUSTDROP_ENV:-$ROOT_DIR/drop-script/.env}"
  [[ -f "$env_file" ]] || die "env file not found: $env_file"

  set -a
  # shellcheck disable=SC1090
  source "$env_file"
  set +a

  if [[ -z "${NETWORK_PRIVATE_KEY:-}" ]]; then
    [[ -n "${SP1_PRIVATE_KEY:-}" ]] || die "SP1_PRIVATE_KEY or NETWORK_PRIVATE_KEY is required"
    export NETWORK_PRIVATE_KEY="$SP1_PRIVATE_KEY"
  fi
}

rpc_url() {
  if [[ -n "${ARBITRUM_SEPOLIA_RPC_URL:-}" ]]; then
    printf '%s' "$ARBITRUM_SEPOLIA_RPC_URL"
  elif [[ -n "${ARBITRUM_SEPOLIA_RPC:-}" ]]; then
    printf '%s' "$ARBITRUM_SEPOLIA_RPC"
  else
    printf '%s' "$DEFAULT_RPC_URL"
  fi
}

build_bin() {
  local package="$1"
  local bin="$2"
  echo "==> build $package --bin $bin"
  (cd "$ROOT_DIR" && CARGO_BUILD_JOBS="$CARGO_BUILD_JOBS" PROTOC="$PROTOC_BIN" cargo build -p "$package" --bin "$bin")
}

vss_execute() {
  build_bin vss-script vss
  echo "==> vss execute"
  (cd "$ROOT_DIR" && PROTOC="$PROTOC_BIN" ./target/debug/vss --execute)
}

vss_prove() {
  load_network_env
  build_bin vss-script evm
  echo "==> vss prove network groth16"
  (cd "$ROOT_DIR" && PROTOC="$PROTOC_BIN" ./target/debug/evm --system groth16)
}

vss_local_contract() {
  echo "==> vss local contract wrapper tests"
  (cd "$ROOT_DIR" && forge test --root guest/vss/contracts --match-contract VSSGroth16Test -vv)
}

vss_preflight() {
  need_cmd jq
  need_cmd cast

  local fixture="$ROOT_DIR/guest/vss/contracts/src/fixtures/groth16-fixture.json"
  [[ -f "$fixture" ]] || die "fixture not found: $fixture"

  local vkey public_values proof
  vkey="$(jq -r .vkey "$fixture")"
  public_values="$(jq -r .publicValues "$fixture")"
  proof="$(jq -r .proof "$fixture")"

  [[ "$vkey" != "null" && "$vkey" != "" ]] || die "fixture vkey is empty"
  [[ "$public_values" != "null" && "$public_values" != "" && "$public_values" != "0x" ]] || die "fixture publicValues is empty"
  [[ "$proof" != "null" && "$proof" != "" && "$proof" != "0x" ]] || die "fixture proof is empty"

  echo "==> vss preflight official SP1 gateway eth_call"
  cast call "$SP1_GATEWAY_GROTH16" "verifyProof(bytes32,bytes,bytes)" "$vkey" "$public_values" "$proof" --rpc-url "$(rpc_url)"
}

vdd_execute() {
  build_bin vdd-script main_walrus_rslhve
  echo "==> vdd walrus_rslhve execute"
  (cd "$ROOT_DIR" && PROTOC="$PROTOC_BIN" VDD_RSLHVE_DATA_SIZE="${VDD_RSLHVE_DATA_SIZE:-65536}" ./target/debug/main_walrus_rslhve --execute)
}

vdd_prove() {
  load_network_env
  build_bin vdd-script evm_walrus_rslhve
  echo "==> vdd walrus_rslhve prove network groth16"
  (cd "$ROOT_DIR" && PROTOC="$PROTOC_BIN" VDD_RSLHVE_DATA_SIZE="${VDD_RSLHVE_DATA_SIZE:-65536}" ./target/debug/evm_walrus_rslhve --system groth16)
}

vdd_local_contract() {
  echo "==> vdd local contract wrapper tests"
  (cd "$ROOT_DIR" && forge test --root guest/vdd/contracts --match-contract VDD_RSLHTest -vv)
}

vdd_preflight() {
  need_cmd jq
  need_cmd cast

  local fixture="$ROOT_DIR/guest/vdd/contracts/src/fixtures/vdd-walrus-rslh-groth16-fixture.json"
  [[ -f "$fixture" ]] || die "fixture not found: $fixture"

  local vkey public_values proof
  vkey="$(jq -r .vkey "$fixture")"
  public_values="$(jq -r .publicValues "$fixture")"
  proof="$(jq -r .proof "$fixture")"

  [[ "$vkey" != "null" && "$vkey" != "" ]] || die "fixture vkey is empty"
  [[ "$public_values" != "null" && "$public_values" != "" && "$public_values" != "0x" ]] || die "fixture publicValues is empty"
  [[ "$proof" != "null" && "$proof" != "" && "$proof" != "0x" ]] || die "fixture proof is empty"

  echo "==> vdd preflight official SP1 gateway eth_call"
  cast call "$SP1_GATEWAY_GROTH16" "verifyProof(bytes32,bytes,bytes)" "$vkey" "$public_values" "$proof" --rpc-url "$(rpc_url)"
}

run_stage() {
  local program="$1"
  local stage="$2"

  case "$program:$stage" in
    vss:execute) vss_execute ;;
    vss:prove) vss_prove ;;
    vss:local-contract) vss_local_contract ;;
    vss:preflight) vss_preflight ;;
    vss:all)
      vss_execute
      vss_prove
      vss_local_contract
      vss_preflight
      ;;
    vdd:execute) vdd_execute ;;
    vdd:prove) vdd_prove ;;
    vdd:local-contract) vdd_local_contract ;;
    vdd:preflight) vdd_preflight ;;
    vdd:all)
      vdd_execute
      vdd_prove
      vdd_local_contract
      vdd_preflight
      ;;
    *) usage; die "unknown program/stage: $program $stage" ;;
  esac
}

main() {
  if [[ "${1:-}" == "help" || "${1:-}" == "--help" || $# -ne 2 ]]; then
    usage
    [[ "${1:-}" == "help" || "${1:-}" == "--help" ]] && exit 0
    exit 1
  fi

  run_stage "$1" "$2"
}

main "$@"
