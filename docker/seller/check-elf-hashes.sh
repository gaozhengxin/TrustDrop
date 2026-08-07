#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

check_hash() {
  local expected="$1"
  local path="$2"
  if [ ! -f "$path" ]; then
    echo "missing ELF: $path" >&2
    exit 10
  fi
  local actual
  actual="$(sha256sum "$path" | awk '{print $1}')"
  if [ "$actual" != "$expected" ]; then
    echo "ELF hash mismatch: $path" >&2
    echo "  expected: $expected" >&2
    echo "  actual:   $actual" >&2
    exit 11
  fi
  echo "ELF ok: $path $actual"
}

check_hash "bbb48442fbb8bfcfbdca83cb5ebb5ca798b3697d9e59abd827d1d8383641bb2e" \
  "guest/vss/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/vss-program"
check_hash "791680137fe92209b774830c7272bda8b7c0c8e53c73345be3e0e51cbc3e69df" \
  "guest/vdd/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/program-vdd-walrus-rslhve"
