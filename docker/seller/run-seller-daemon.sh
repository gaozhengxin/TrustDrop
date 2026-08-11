#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
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

load_env_file() {
  local file="$1"
  [ -f "$file" ] || return 0
  while IFS='=' read -r key value; do
    case "$key" in ''|'#'*) continue ;; esac
    value="${value%$'\r'}"
    if [[ "$value" == \"*\" ]]; then value="${value:1:${#value}-2}"; fi
    if [[ "$value" == \'*\' ]]; then value="${value:1:${#value}-2}"; fi
    export "$key=$value"
  done < "$file"
}

expand_path() {
  local value="$1"
  case "$value" in
    "~") printf '%s\n' "$HOME" ;;
    "~/"*) printf '%s/%s\n' "$HOME" "${value#~/}" ;;
    *) printf '%s\n' "$value" ;;
  esac
}

load_env_file "${SCRIPT_DIR}/seller.env"

export TRUSTDROP_SUI_DIR="$(expand_path "${TRUSTDROP_SUI_DIR:-$HOME/.sui}")"
if [ -n "${TRUSTDROP_WALRUS_CONFIG_DIR:-}" ]; then
  export TRUSTDROP_WALRUS_CONFIG_DIR="$(expand_path "$TRUSTDROP_WALRUS_CONFIG_DIR")"
elif [ -n "${TRUSTDROP_WALRUS_DIR:-}" ]; then
  # Backward-compatible alias used by older local seller.env files.
  export TRUSTDROP_WALRUS_CONFIG_DIR="$(expand_path "$TRUSTDROP_WALRUS_DIR")"
else
  export TRUSTDROP_WALRUS_CONFIG_DIR="$(expand_path "$HOME/walrus")"
fi
export TRUSTDROP_CARGO_HOME="$(expand_path "${TRUSTDROP_CARGO_HOME:-$HOME/.cargo-trustdrop-justin}")"
export TRUSTDROP_STATE_DIR="$(expand_path "${TRUSTDROP_STATE_DIR:-$HOME/.trustdrop}")"
export TRUSTDROP_HOST_INPUT_DIR="$(expand_path "${TRUSTDROP_HOST_INPUT_DIR:-$HOME/TrustDrop/TrustDrop/app/gui/demo-assets}")"
mkdir -p "$TRUSTDROP_STATE_DIR"

for path in "$TRUSTDROP_SUI_DIR/sui_config/client.yaml" "$TRUSTDROP_WALRUS_CONFIG_DIR/client_config.yaml" "$TRUSTDROP_CARGO_HOME" "$TRUSTDROP_STATE_DIR" "$TRUSTDROP_HOST_INPUT_DIR"; do
  if [ ! -e "$path" ]; then
    echo "missing required path: $path" >&2
    exit 2
  fi
done

echo "Walrus publisher binary: packaged in trustdrop/walrus-publisher image"
echo "Walrus config mount: ${TRUSTDROP_WALRUS_CONFIG_DIR} -> /home/justin/walrus:ro"
echo "Sui wallet mount: ${TRUSTDROP_SUI_DIR} -> /home/justin/.sui"
echo "Seller state mount: ${TRUSTDROP_STATE_DIR} -> /root/.trustdrop"
echo "Host input mount: ${TRUSTDROP_HOST_INPUT_DIR} -> /host-input:ro"

"${SCRIPT_DIR}/check-elf-hashes.sh"

cd "$SCRIPT_DIR"
exec "$DOCKER_BIN" compose --env-file /dev/null up --build seller-daemon
