#!/usr/bin/env bash
set -u
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

STRICT=0
JSON=0
SECTION="all"

PASS_COUNT=0
WARN_COUNT=0
ACTION_COUNT=0
INFO_COUNT=0
RESULTS=()

usage() {
  cat <<'USAGE'
Usage:
  drop-script/scripts/check-env.sh [--strict] [--json] [--section SECTION]

Sections:
  all          Run every readonly check.
  tools        Repository, required tools, and protoc checks.
  env          .env file and variable presence checks.
  accounts     Arbitrum Sepolia chain id and account balance checks.
  contracts    Deployment address consistency and contract code checks.
  sp1          Guest proof fixture, ELF, and verifier preflight checks.
  walrus       Local Walrus publisher connectivity and mainnet manual gate.
  oracle       Hybrid OracleProxy readonly checks.
  subgraph     Subgraph env, manifest, codegen, and build checks.
  drop-script  Input asset and drop-script cargo check.
  manual       Manual action checklist only.

Default behavior is readonly:
  - no transactions
  - no contract deployment
  - no subgraph deployment
  - no SP1 prove request
  - no Walrus upload
  - no secret printing
USAGE
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --strict)
      STRICT=1
      shift
      ;;
    --json)
      JSON=1
      shift
      ;;
    --section)
      if [ "$#" -lt 2 ]; then
        echo "missing value for --section" >&2
        exit 2
      fi
      SECTION="$2"
      shift 2
      ;;
    -h|--help|help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

cd "$ROOT_DIR" || exit 1

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

record() {
  local status="$1"
  local area="$2"
  local message="$3"
  case "$status" in
    PASS) PASS_COUNT=$((PASS_COUNT + 1)) ;;
    WARN) WARN_COUNT=$((WARN_COUNT + 1)) ;;
    ACTION_REQUIRED) ACTION_COUNT=$((ACTION_COUNT + 1)) ;;
    INFO) INFO_COUNT=$((INFO_COUNT + 1)) ;;
  esac
  RESULTS+=("$status|$area|$message")
  if [ "$JSON" -eq 0 ]; then
    printf '[%s] %s: %s\n' "$status" "$area" "$message"
  fi
}

pass() { record PASS "$1" "$2"; }
warn() { record WARN "$1" "$2"; }
action() { record ACTION_REQUIRED "$1" "$2"; }
info() { record INFO "$1" "$2"; }

run_section() {
  [ "$SECTION" = "all" ] || [ "$SECTION" = "$1" ]
}

have_cmd() {
  command -v "$1" >/dev/null 2>&1
}

load_env_file() {
  local file="$1"
  if [ -f "$file" ]; then
    set -a
    # shellcheck disable=SC1090
    . "$file"
    set +a
    return 0
  fi
  return 1
}

env_has_key() {
  local file="$1"
  local key="$2"
  [ -f "$file" ] && grep -Eq "^[[:space:]]*(export[[:space:]]+)?${key}=" "$file"
}

require_env_key() {
  local area="$1"
  local file="$2"
  local key="$3"
  if env_has_key "$file" "$key"; then
    pass "$area" "$key is present in $file"
  else
    action "$area" "$key is missing in $file"
  fi
}

optional_env_key() {
  local area="$1"
  local file="$2"
  local key="$3"
  if env_has_key "$file" "$key"; then
    pass "$area" "$key is present in $file"
  else
    warn "$area" "$key is not set in $file"
  fi
}

first_env_value() {
  local key
  for key in "$@"; do
    if [ -n "${!key:-}" ]; then
      printf '%s' "${!key}"
      return 0
    fi
  done
  return 1
}

is_address() {
  printf '%s' "$1" | grep -Eq '^0x[0-9a-fA-F]{40}$'
}

normalize_addr() {
  printf '%s' "$1" | tr '[:upper:]' '[:lower:]'
}

find_deployed_addr() {
  local label="$1"
  awk -F'|' -v label="$label" '
    tolower($2) ~ tolower(label) {
      print $0
      exit
    }
  ' contracts/deployed.md 2>/dev/null | grep -Eo '0x[0-9a-fA-F]{40}' | head -1 || true
}

json_broadcast_addr() {
  local contract="$1"
  if [ ! -f contracts/broadcast/DeployMain.s.sol/421614/run-latest.json ]; then
    return 0
  fi
  if have_cmd jq; then
    jq -r --arg name "$contract" '
      [.transactions[]? | select(.contractName == $name and .contractAddress != null) | .contractAddress] | last // empty
    ' contracts/broadcast/DeployMain.s.sol/421614/run-latest.json 2>/dev/null
  fi
}

yaml_hub_addr() {
  awk '
    $1 == "address:" {
      gsub(/"/, "", $2)
      print $2
      exit
    }
  ' subgraph/subgraph.yaml 2>/dev/null | grep -Eo '0x[0-9a-fA-F]{40}' | head -1 || true
}

yaml_start_block() {
  awk '$1 == "startBlock:" { print $2; exit }' subgraph/subgraph.yaml 2>/dev/null || true
}

wallet_addr_from_key() {
  local key="$1"
  if ! have_cmd cast; then
    return 1
  fi
  cast wallet address --private-key "$key" 2>/dev/null
}

eth_balance_wei() {
  local addr="$1"
  local rpc="$2"
  cast balance "$addr" --rpc-url "$rpc" 2>/dev/null
}

wei_at_least_eth() {
  local wei="$1"
  local eth_dec="$2"
  local threshold
  threshold="$(awk -v e="$eth_dec" 'BEGIN { printf "%.0f", e * 1000000000000000000 }')"
  awk -v a="$wei" -v b="$threshold" 'BEGIN { exit !(a >= b) }'
}

check_code() {
  local area="$1"
  local name="$2"
  local addr="$3"
  local rpc="$4"
  if ! is_address "$addr"; then
    action "$area" "$name address is missing or invalid"
    return
  fi
  local code
  code="$(cast code "$addr" --rpc-url "$rpc" 2>/dev/null || true)"
  if [ -n "$code" ] && [ "$code" != "0x" ]; then
    pass "$area" "$name has code at $addr"
  else
    action "$area" "$name has no code at $addr"
  fi
}

check_eq_addr() {
  local area="$1"
  local label="$2"
  local left_name="$3"
  local left="$4"
  local right_name="$5"
  local right="$6"
  if [ -z "$left" ] || [ -z "$right" ]; then
    warn "$area" "$label cannot be compared because one side is missing"
    return
  fi
  if [ "$(normalize_addr "$left")" = "$(normalize_addr "$right")" ]; then
    pass "$area" "$label matches between $left_name and $right_name"
  else
    action "$area" "$label mismatch: $left_name=$left, $right_name=$right"
  fi
}

check_tools() {
  info tools "repository root: $ROOT_DIR"
  if [ -f Cargo.toml ] && [ -d drop-script ] && [ -d contracts ]; then
    pass tools "project root looks correct"
  else
    action tools "current root does not look like TrustDrop/TrustDrop"
  fi

  local status
  status="$(git status --short 2>/dev/null || true)"
  if [ -z "$status" ]; then
    pass tools "git worktree is clean"
  else
    warn tools "git worktree has uncommitted changes; confirm before full-flow testing"
  fi

  local cmd
  for cmd in cargo forge cast pnpm node curl; do
    if have_cmd "$cmd"; then
      pass tools "$cmd is available"
    else
      action tools "$cmd is not available"
    fi
  done

  if have_cmd jq; then
    pass tools "jq is available"
  else
    warn tools "jq is not available; broadcast JSON checks will be limited"
  fi

  local protoc_path="${PROTOC:-/tmp/protoc-25.3/bin/protoc}"
  if [ -x "$protoc_path" ]; then
    pass tools "protoc is available at $protoc_path"
  else
    action tools "protoc not found at $protoc_path; set PROTOC or install /tmp/protoc-25.3/bin/protoc"
  fi
}

check_env_files() {
  if load_env_file drop-script/.env; then
    pass env "drop-script/.env exists"
  else
    action env "drop-script/.env is missing"
  fi
  if [ -f contracts/.env ]; then
    pass env "contracts/.env exists"
  else
    action env "contracts/.env is missing"
  fi
  if [ -f subgraph/.env ]; then
    pass env "subgraph/.env exists"
  else
    action env "subgraph/.env is missing"
  fi

  if [ -f drop-script/.env ]; then
    require_env_key env drop-script/.env SELLER_KEY
    require_env_key env drop-script/.env BUYER_KEY
    require_env_key env drop-script/.env SP1_PRIVATE_KEY
    if env_has_key drop-script/.env ARBITRUM_SEPOLIA_RPC || env_has_key drop-script/.env ARBITRUM_SEPOLIA_RPC_URL; then
      pass env "Arbitrum Sepolia RPC is present in drop-script/.env"
    else
      action env "ARBITRUM_SEPOLIA_RPC or ARBITRUM_SEPOLIA_RPC_URL is missing in drop-script/.env"
    fi
    optional_env_key env drop-script/.env WALRUS_LOCAL_ENDPOINT
    require_env_key env drop-script/.env HUB_ADDRESS
    require_env_key env drop-script/.env VSS_VERIFIER_ADDRESS
    require_env_key env drop-script/.env VDD_VERIFIER_ADDRESS
    optional_env_key env drop-script/.env DROP_ORACLE_TIMEOUT_SECS
    optional_env_key env drop-script/.env ORACLE_MODE
    if [ "${ORACLE_MODE:-external}" = "centralized" ]; then
      require_env_key env drop-script/.env ORACLE_WORKER_URL
      require_env_key env drop-script/.env ORACLE_WORKER_TOKEN
      optional_env_key env drop-script/.env ORACLE_WORKER_STATUS_URL
    else
      warn env "ORACLE_MODE is '${ORACLE_MODE:-external}'; drop-script will not trigger centralized Worker automatically"
    fi
  fi

  if [ -f contracts/.env ]; then
    if env_has_key contracts/.env ARBITRUM_SEPOLIA_RPC || env_has_key contracts/.env ARBITRUM_SEPOLIA_RPC_URL; then
      pass env "Arbitrum Sepolia RPC is present in contracts/.env"
    else
      action env "Arbitrum Sepolia RPC is missing in contracts/.env"
    fi
    if grep -Eq '^[[:space:]]*(export[[:space:]]+)?(PRIVATE_KEY|DEPLOYER_PRIVATE_KEY|DEPLOY_PRIVATE_KEY)=' contracts/.env; then
      pass env "deployment private key variable is present in contracts/.env"
    else
      warn env "deployment private key variable not recognized in contracts/.env"
    fi
    if grep -Eq '^[[:space:]]*(export[[:space:]]+)?ORACLE_RELAYER_PRIVATE_KEY=' contracts/.env; then
      pass env "ORACLE_RELAYER_PRIVATE_KEY is present in contracts/.env"
    else
      action env "ORACLE_RELAYER_PRIVATE_KEY is missing in contracts/.env; add the Worker signer private key locally"
    fi
    if grep -Eq '^[[:space:]]*(export[[:space:]]+)?ORACLE_PROXY_ADDRESS=' contracts/.env; then
      pass env "ORACLE_PROXY_ADDRESS is present in contracts/.env"
    else
      warn env "ORACLE_PROXY_ADDRESS is not configured; signer setup script uses the documented latest OracleProxy default"
    fi
    if grep -Eq '^[[:space:]]*(export[[:space:]]+)?CENTRALIZED_ORACLE_SIGNER=' contracts/.env; then
      pass env "CENTRALIZED_ORACLE_SIGNER is present in contracts/.env"
    else
      warn env "CENTRALIZED_ORACLE_SIGNER is optional; SetCentralizedOracleSigner derives it from ORACLE_RELAYER_PRIVATE_KEY"
    fi
    if grep -Eq '^[[:space:]]*(export[[:space:]]+)?CRE_FORWARDER=' contracts/.env; then
      pass env "CRE_FORWARDER is present in contracts/.env"
    else
      warn env "CRE_FORWARDER is not configured; DeployMain uses the documented Arbitrum Sepolia default"
    fi
  fi

  if [ -f subgraph/.env ]; then
    require_env_key env subgraph/.env SUBGRAPH_SLUG
    require_env_key env subgraph/.env DEPLOY_KEY
  fi
}

ensure_runtime_env() {
  load_env_file drop-script/.env >/dev/null 2>&1 || true
  RPC_URL="$(first_env_value ARBITRUM_SEPOLIA_RPC ARBITRUM_SEPOLIA_RPC_URL || true)"
  RPC_URL="${RPC_URL:-https://sepolia-rollup.arbitrum.io/rpc}"
  WALRUS_ENDPOINT="${WALRUS_LOCAL_ENDPOINT:-http://localhost:31415}"
}

check_accounts() {
  ensure_runtime_env
  if ! have_cmd cast; then
    action accounts "cast is required for account checks"
    return
  fi
  local chain_id
  chain_id="$(cast chain-id --rpc-url "$RPC_URL" 2>/dev/null || true)"
  if [ "$chain_id" = "421614" ]; then
    pass accounts "RPC chain id is Arbitrum Sepolia 421614"
  else
    action accounts "RPC chain id is '$chain_id', expected 421614"
  fi

  local role key addr balance min
  for role in SELLER BUYER SP1_PROVER; do
    case "$role" in
      SELLER) key="${SELLER_KEY:-}"; min="0.01" ;;
      BUYER) key="${BUYER_KEY:-}"; min="0.01" ;;
      SP1_PROVER) key="${SP1_PRIVATE_KEY:-}"; min="0.005" ;;
    esac
    if [ -z "$key" ]; then
      action accounts "$role private key is missing; add it to drop-script/.env"
      continue
    fi
    addr="$(wallet_addr_from_key "$key" || true)"
    if ! is_address "$addr"; then
      action accounts "could not derive $role address from configured key"
      continue
    fi
    pass accounts "$role address derived: $addr"
    balance="$(eth_balance_wei "$addr" "$RPC_URL" || true)"
    if printf '%s' "$balance" | grep -Eq '^[0-9]+$'; then
      if wei_at_least_eth "$balance" "$min"; then
        pass accounts "$role balance is at least $min ETH"
      else
        action accounts "$role balance is below $min ETH; fund $addr on Arbitrum Sepolia"
      fi
    else
      action accounts "could not read $role ETH balance from RPC"
    fi
  done

  warn accounts "PROVE token balance/allowance is not checked automatically; confirm Prove Network dashboard/allowance manually"
}

check_contracts() {
  ensure_runtime_env
  if ! have_cmd cast; then
    action contracts "cast is required for contract checks"
    return
  fi

  local deployed_hub deployed_vss deployed_vdd deployed_oracle deployed_logic
  deployed_hub="$(find_deployed_addr "Exchange hub")"
  deployed_vss="$(find_deployed_addr "VSS")"
  deployed_vdd="$(find_deployed_addr "VDD")"
  deployed_oracle="$(find_deployed_addr "Oracle proxy")"
  deployed_logic="$(find_deployed_addr "Exchange logic")"

  check_code contracts "ExchangeHub" "${HUB_ADDRESS:-}" "$RPC_URL"
  check_code contracts "VSS verifier" "${VSS_VERIFIER_ADDRESS:-}" "$RPC_URL"
  check_code contracts "VDD verifier" "${VDD_VERIFIER_ADDRESS:-}" "$RPC_URL"
  check_code contracts "OracleProxy" "$deployed_oracle" "$RPC_URL"
  check_code contracts "Exchange logic" "$deployed_logic" "$RPC_URL"

  check_eq_addr contracts "Hub address" "drop-script/.env" "${HUB_ADDRESS:-}" "contracts/deployed.md" "$deployed_hub"
  check_eq_addr contracts "VSS verifier address" "drop-script/.env" "${VSS_VERIFIER_ADDRESS:-}" "contracts/deployed.md" "$deployed_vss"
  check_eq_addr contracts "VDD verifier address" "drop-script/.env" "${VDD_VERIFIER_ADDRESS:-}" "contracts/deployed.md" "$deployed_vdd"

  local subgraph_hub start_block
  subgraph_hub="$(yaml_hub_addr)"
  start_block="$(yaml_start_block)"
  check_eq_addr contracts "Subgraph Hub address" "subgraph/subgraph.yaml" "$subgraph_hub" "contracts/deployed.md" "$deployed_hub"
  if [ -n "$start_block" ]; then
    pass contracts "subgraph startBlock is set to $start_block"
  else
    warn contracts "subgraph startBlock not found"
  fi

  if have_cmd jq; then
    check_eq_addr contracts "Broadcast Hub address" "broadcast" "$(json_broadcast_addr ExchangeHub)" "contracts/deployed.md" "$deployed_hub"
    check_eq_addr contracts "Broadcast OracleProxy address" "broadcast" "$(json_broadcast_addr OracleProxy)" "contracts/deployed.md" "$deployed_oracle"
    check_eq_addr contracts "Broadcast Exchange logic address" "broadcast" "$(json_broadcast_addr ExchangeChannelImplementation)" "contracts/deployed.md" "$deployed_logic"
  fi

  local hub_impl hub_oracle hub_vss hub_vdd oracle_controller oracle_forwarder oracle_signer oracle_mode
  hub_impl="$(cast call "${HUB_ADDRESS:-0x0000000000000000000000000000000000000000}" 'implementation()(address)' --rpc-url "$RPC_URL" 2>/dev/null || true)"
  hub_oracle="$(cast call "${HUB_ADDRESS:-0x0000000000000000000000000000000000000000}" 'oracleWrapper()(address)' --rpc-url "$RPC_URL" 2>/dev/null || true)"
  hub_vss="$(cast call "${HUB_ADDRESS:-0x0000000000000000000000000000000000000000}" 'vssVerifier()(address)' --rpc-url "$RPC_URL" 2>/dev/null || true)"
  hub_vdd="$(cast call "${HUB_ADDRESS:-0x0000000000000000000000000000000000000000}" 'vddVerifier()(address)' --rpc-url "$RPC_URL" 2>/dev/null || true)"
  oracle_controller="$(cast call "$deployed_oracle" 'controller()(address)' --rpc-url "$RPC_URL" 2>/dev/null || true)"
  oracle_forwarder="$(cast call "$deployed_oracle" 'creForwarder()(address)' --rpc-url "$RPC_URL" 2>/dev/null || true)"
  oracle_signer="$(cast call "$deployed_oracle" 'centralizedOracleSigner()(address)' --rpc-url "$RPC_URL" 2>/dev/null || true)"
  oracle_mode="$(cast call "$deployed_oracle" 'defaultMode()(uint8)' --rpc-url "$RPC_URL" 2>/dev/null || true)"

  check_eq_addr contracts "Hub implementation" "Hub" "$hub_impl" "contracts/deployed.md" "$deployed_logic"
  check_eq_addr contracts "Hub oracleWrapper" "Hub" "$hub_oracle" "contracts/deployed.md" "$deployed_oracle"
  check_eq_addr contracts "Hub VSS verifier" "Hub" "$hub_vss" "contracts/deployed.md" "$deployed_vss"
  check_eq_addr contracts "Hub VDD verifier" "Hub" "$hub_vdd" "contracts/deployed.md" "$deployed_vdd"
  check_eq_addr contracts "OracleProxy controller" "OracleProxy" "$oracle_controller" "contracts/deployed.md Hub" "$deployed_hub"
  check_eq_addr contracts "OracleProxy CRE forwarder" "OracleProxy" "$oracle_forwarder" "Arbitrum Sepolia CRE forwarder" "0x76c9cf548b4179F8901cda1f8623568b58215E62"

  if [ "$(normalize_addr "$oracle_signer")" = "0x0000000000000000000000000000000000000000" ]; then
    action contracts "centralizedOracleSigner is not set; configure it after the Worker private key is ready"
  elif is_address "$oracle_signer"; then
    pass contracts "centralizedOracleSigner is configured"
  else
    action contracts "could not read centralizedOracleSigner"
  fi
  if [ "$oracle_mode" = "0" ]; then
    pass contracts "OracleProxy defaultMode is Centralized"
  else
    action contracts "OracleProxy defaultMode is '$oracle_mode', expected 0 for centralized oracle prototype"
  fi
}

check_sp1() {
  if [ -x guest/scripts/zk-proof-test.sh ]; then
    pass sp1 "guest proof workflow script is executable"
  else
    action sp1 "guest/scripts/zk-proof-test.sh is missing or not executable"
  fi
  if [ -s guest/vss/contracts/src/fixtures/groth16-fixture.json ]; then
    pass sp1 "VSS Groth16 fixture exists"
  else
    action sp1 "VSS Groth16 fixture is missing"
  fi
  if [ -s guest/vdd/contracts/src/fixtures/vdd-walrus-rslh-groth16-fixture.json ]; then
    pass sp1 "VDD Groth16 fixture exists"
  else
    action sp1 "VDD Groth16 fixture is missing"
  fi
  if [ -s guest/vss/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/vss-program ]; then
    pass sp1 "VSS ELF exists for drop-script include_bytes"
  else
    action sp1 "VSS ELF is missing; rebuild vss-program"
  fi
  if [ -s guest/vdd/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/program-vdd-walrus-rslhve ]; then
    pass sp1 "VDD ELF exists for drop-script include_bytes"
  else
    action sp1 "VDD ELF is missing; rebuild program-vdd-walrus-rslhve"
  fi

  if [ -x guest/scripts/zk-proof-test.sh ]; then
    if guest/scripts/zk-proof-test.sh vss preflight >/tmp/trustdrop-vss-preflight.log 2>&1; then
      pass sp1 "VSS official gateway preflight passed"
    else
      action sp1 "VSS official gateway preflight failed; inspect /tmp/trustdrop-vss-preflight.log"
    fi
    if guest/scripts/zk-proof-test.sh vdd preflight >/tmp/trustdrop-vdd-preflight.log 2>&1; then
      pass sp1 "VDD official gateway preflight passed"
    else
      action sp1 "VDD official gateway preflight failed; inspect /tmp/trustdrop-vdd-preflight.log"
    fi
  fi
  warn sp1 "checklist does not run execute or prove; run guest proof workflow explicitly if fixtures need refresh"
}

check_walrus() {
  ensure_runtime_env
  if [ -x /home/justin/walrus/start.sh ]; then
    pass walrus "/home/justin/walrus/start.sh exists and is executable"
  elif [ -f /home/justin/walrus/start.sh ]; then
    warn walrus "/home/justin/walrus/start.sh exists but is not executable"
  else
    action walrus "/home/justin/walrus/start.sh is missing"
  fi

  local http_code
  http_code="$(curl -sS -o /dev/null -w '%{http_code}' "$WALRUS_ENDPOINT" 2>/dev/null || true)"
  case "$http_code" in
    2*|404)
      pass walrus "Walrus endpoint is reachable at $WALRUS_ENDPOINT with HTTP $http_code"
      ;;
    000|"")
      action walrus "Walrus endpoint is not reachable at $WALRUS_ENDPOINT; start /home/justin/walrus/start.sh"
      ;;
    *)
      warn walrus "Walrus endpoint returned HTTP $http_code at $WALRUS_ENDPOINT"
      ;;
  esac
  action walrus "manual confirmation required: Walrus publisher must be mainnet, with usable wallet balance/storage quota"
  warn walrus "default checklist does not upload a test blob or consume Walrus storage"
}

check_oracle() {
  ensure_runtime_env
  load_env_file contracts/.env >/dev/null 2>&1 || true
  if ! have_cmd cast; then
    action oracle "cast is required for oracle checks"
    return
  fi
  local deployed_oracle deployed_hub
  deployed_oracle="$(find_deployed_addr "Oracle proxy")"
  deployed_hub="$(find_deployed_addr "Exchange hub")"
  check_code oracle "OracleProxy" "$deployed_oracle" "$RPC_URL"

  local controller signer forwarder mode
  controller="$(cast call "$deployed_oracle" 'controller()(address)' --rpc-url "$RPC_URL" 2>/dev/null || true)"
  signer="$(cast call "$deployed_oracle" 'centralizedOracleSigner()(address)' --rpc-url "$RPC_URL" 2>/dev/null || true)"
  forwarder="$(cast call "$deployed_oracle" 'creForwarder()(address)' --rpc-url "$RPC_URL" 2>/dev/null || true)"
  mode="$(cast call "$deployed_oracle" 'defaultMode()(uint8)' --rpc-url "$RPC_URL" 2>/dev/null || true)"

  check_eq_addr oracle "OracleProxy controller" "OracleProxy" "$controller" "ExchangeHub" "$deployed_hub"
  check_eq_addr oracle "OracleProxy CRE forwarder" "OracleProxy" "$forwarder" "Arbitrum Sepolia CRE forwarder" "0x76c9cf548b4179F8901cda1f8623568b58215E62"

  local expected_signer
  expected_signer=""
  if [ -n "${ORACLE_RELAYER_PRIVATE_KEY:-}" ]; then
    expected_signer="$(wallet_addr_from_key "$ORACLE_RELAYER_PRIVATE_KEY" || true)"
    if is_address "$expected_signer"; then
      pass oracle "ORACLE_RELAYER_PRIVATE_KEY derives Worker signer $expected_signer"
    else
      action oracle "could not derive Worker signer from ORACLE_RELAYER_PRIVATE_KEY"
    fi
  else
    action oracle "ORACLE_RELAYER_PRIVATE_KEY is missing in contracts/.env"
  fi

  if [ "$(normalize_addr "$signer")" = "0x0000000000000000000000000000000000000000" ]; then
    action oracle "centralizedOracleSigner is not set; prepare Worker private key, then owner calls setCentralizedOracleSigner"
  elif is_address "$signer"; then
    pass oracle "centralizedOracleSigner is configured"
    if is_address "$expected_signer"; then
      check_eq_addr oracle "centralizedOracleSigner" "OracleProxy" "$signer" "ORACLE_RELAYER_PRIVATE_KEY" "$expected_signer"
    fi
  else
    action oracle "could not read centralizedOracleSigner"
  fi
  if [ "$mode" = "0" ]; then
    pass oracle "OracleProxy defaultMode is Centralized"
  else
    action oracle "OracleProxy defaultMode is '$mode', expected 0 for centralized oracle prototype"
  fi

  if [ "${ORACLE_MODE:-external}" = "centralized" ]; then
    if [ -n "${ORACLE_WORKER_URL:-}" ] && [ -n "${ORACLE_WORKER_TOKEN:-}" ]; then
      local status_url status_json status_ok balance_ok pending_ok
      status_url="${ORACLE_WORKER_STATUS_URL:-${ORACLE_WORKER_URL%/}/status}"
      status_json="$(curl -sS -H "Authorization: Bearer ${ORACLE_WORKER_TOKEN}" "$status_url" 2>/dev/null || true)"
      status_ok="$(printf '%s' "$status_json" | grep -Eo '"ok"[[:space:]]*:[[:space:]]*true' || true)"
      balance_ok="$(printf '%s' "$status_json" | grep -Eo '"relayerBalanceSufficient"[[:space:]]*:[[:space:]]*true' || true)"
      pending_ok="$(printf '%s' "$status_json" | grep -Eo '"relayerHasPendingTx"[[:space:]]*:[[:space:]]*false' || true)"
      if [ -n "$status_ok" ]; then
        pass oracle "centralized Oracle Worker status reports ok"
      else
        action oracle "centralized Oracle Worker status is not ok or not reachable"
      fi
      if [ -n "$balance_ok" ]; then
        pass oracle "centralized Oracle Worker reports relayer balance sufficient"
      else
        action oracle "centralized Oracle Worker reports relayer balance insufficient or unknown"
      fi
      if [ -n "$pending_ok" ]; then
        pass oracle "centralized Oracle Worker reports no pending relayer tx"
      else
        action oracle "centralized Oracle Worker reports pending relayer tx or unknown nonce state"
      fi
    else
      action oracle "ORACLE_MODE=centralized requires ORACLE_WORKER_URL and ORACLE_WORKER_TOKEN in drop-script/.env"
    fi
  else
    action oracle "centralized Oracle Worker is not enabled in drop-script/.env; set ORACLE_MODE=centralized after Worker deployment"
  fi
  info oracle "CRE-compatible onReport path is present but not used by the current prototype"
  warn oracle "cCipher bytes-to-Walrus-id encoding remains a full-flow debugging risk"
}

check_subgraph() {
  if [ -f subgraph/.env ]; then
    pass subgraph "subgraph/.env exists"
  else
    action subgraph "subgraph/.env is missing"
  fi
  if [ -f subgraph/subgraph.yaml ]; then
    pass subgraph "subgraph/subgraph.yaml exists"
  else
    action subgraph "subgraph/subgraph.yaml is missing"
  fi
  if [ -f subgraph/.env ]; then
    require_env_key subgraph subgraph/.env SUBGRAPH_SLUG
    require_env_key subgraph subgraph/.env DEPLOY_KEY
  fi

  local deployed_hub subgraph_hub start_block
  deployed_hub="$(find_deployed_addr "Exchange hub")"
  subgraph_hub="$(yaml_hub_addr)"
  start_block="$(yaml_start_block)"
  check_eq_addr subgraph "Subgraph Hub address" "subgraph/subgraph.yaml" "$subgraph_hub" "contracts/deployed.md" "$deployed_hub"
  if [ -n "$start_block" ]; then
    pass subgraph "subgraph startBlock is $start_block"
  else
    warn subgraph "subgraph startBlock is missing"
  fi

  if have_cmd pnpm; then
    if pnpm --dir subgraph codegen >/tmp/trustdrop-subgraph-codegen.log 2>&1; then
      pass subgraph "pnpm --dir subgraph codegen passed"
    else
      action subgraph "subgraph codegen failed; inspect /tmp/trustdrop-subgraph-codegen.log"
    fi
    if pnpm --dir subgraph build >/tmp/trustdrop-subgraph-build.log 2>&1; then
      pass subgraph "pnpm --dir subgraph build passed"
    else
      action subgraph "subgraph build failed; inspect /tmp/trustdrop-subgraph-build.log"
    fi
  else
    action subgraph "pnpm is required for subgraph checks"
  fi
  warn subgraph "checklist does not deploy subgraph; deploy only after explicit approval"
}

check_drop_script() {
  local protoc_path="${PROTOC:-/tmp/protoc-25.3/bin/protoc}"
  local input_asset="drop-script/KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile.mp4"
  if [ -s "$input_asset" ]; then
    pass drop-script "input asset $input_asset exists"
  else
    action drop-script "input asset $input_asset is missing"
  fi
  if [ -x "$protoc_path" ]; then
    if PROTOC="$protoc_path" cargo check -p drop-script >/tmp/trustdrop-drop-script-check.log 2>&1; then
      pass drop-script "cargo check -p drop-script passed"
    else
      action drop-script "cargo check -p drop-script failed; inspect /tmp/trustdrop-drop-script-check.log"
    fi
  else
    action drop-script "cannot run cargo check because protoc is missing at $protoc_path"
  fi
  warn drop-script "checklist does not run drop-script full flow; that requires explicit approval"
}

manual_checklist() {
  info manual "Confirm Arbitrum Sepolia chain id 421614 for this iteration."
  info manual "Confirm Walrus publisher uses mainnet, not testnet or a mock."
  info manual "Confirm /home/justin/walrus/start.sh is started with the intended key."
  info manual "Confirm Walrus/Sui wallet has enough balance or storage quota."
  info manual "Confirm drop-script/.env seller, buyer, and SP1 keys are allowed for this test."
  info manual "Confirm seller and buyer addresses have Arbitrum Sepolia ETH."
  info manual "Confirm SP1 Prove Network key has balance and PROVE allowance."
  info manual "Confirm Hub/VSS/VDD/Oracle addresses use the intended deployment."
  info manual "Confirm centralized Oracle Worker is deployed with its private key configured."
  info manual "Confirm centralizedOracleSigner is set to the Worker signer address."
  info manual "Confirm Worker signer has enough Arbitrum Sepolia ETH."
  info manual "Confirm Worker status page reports ready without exposing balances or secrets."
  info manual "Confirm CRE forwarder remains configured for future compatibility."
  info manual "Confirm subgraph Studio slug and deploy key are valid."
  info manual "Approve separately before running drop-script full flow, transactions, deployment, proof, or Walrus upload."
}

case "$SECTION" in
  all|tools|env|accounts|contracts|sp1|walrus|oracle|subgraph|drop-script|manual) ;;
  *)
    echo "unknown section: $SECTION" >&2
    usage >&2
    exit 2
    ;;
esac

run_section tools && check_tools
run_section env && check_env_files
run_section accounts && check_accounts
run_section contracts && check_contracts
run_section sp1 && check_sp1
run_section walrus && check_walrus
run_section oracle && check_oracle
run_section subgraph && check_subgraph
run_section drop-script && check_drop_script
run_section manual && manual_checklist

if [ "$JSON" -eq 1 ]; then
  printf '{\n'
  printf '  "summary": {"pass": %s, "warn": %s, "action_required": %s, "info": %s},\n' \
    "$PASS_COUNT" "$WARN_COUNT" "$ACTION_COUNT" "$INFO_COUNT"
  printf '  "results": [\n'
  for i in "${!RESULTS[@]}"; do
    IFS='|' read -r status area message <<<"${RESULTS[$i]}"
    printf '    {"status": "%s", "area": "%s", "message": "%s"}' \
      "$(json_escape "$status")" "$(json_escape "$area")" "$(json_escape "$message")"
    if [ "$i" -lt $((${#RESULTS[@]} - 1)) ]; then
      printf ','
    fi
    printf '\n'
  done
  printf '  ]\n'
  printf '}\n'
else
  printf '\nSummary: PASS=%s WARN=%s ACTION_REQUIRED=%s INFO=%s\n' \
    "$PASS_COUNT" "$WARN_COUNT" "$ACTION_COUNT" "$INFO_COUNT"
fi

if [ "$ACTION_COUNT" -gt 0 ]; then
  exit 1
fi
if [ "$STRICT" -eq 1 ] && [ "$WARN_COUNT" -gt 0 ]; then
  exit 1
fi
exit 0
