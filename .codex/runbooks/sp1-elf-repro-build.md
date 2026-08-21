# SP1 guest ELF reproducible build runbook

This runbook records the Mac mini containerized build method that reproduced the Ubuntu reference TrustDrop guest ELFs byte-for-byte.

## Scope

Build only the SP1 guest ELF artifacts used by the TrustDrop protocol:

- VSS key-sharing guest: `guest/vss/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/vss-program`
- VDD Walrus RSLH/VE guest: `guest/vdd/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/program-vdd-walrus-rslhve`

This runbook does not run `execute`, generate proofs, submit to SP1 Prove Network, deploy contracts, or use private keys.

## Reference environment

Known-good Ubuntu reference:

- Repository path: `/home/justin/TrustDrop/TrustDrop`
- Commit: `53508829edd5c1b5c63b02b74d469e8d5c3be920`
- `cargo-prove sp1 (98a376e 2026-05-15T03:56:11.493352970Z)`
- `cargo 1.95.0 (f2d3ce0bd 2026-03-21)`
- `rustc +succinct --version`: `rustc 1.93.0-dev`

Expected ELF hashes:

```text
bbb48442fbb8bfcfbdca83cb5ebb5ca798b3697d9e59abd827d1d8383641bb2e  guest/vss/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/vss-program
791680137fe92209b774830c7272bda8b7c0c8e53c73345be3e0e51cbc3e69df  guest/vdd/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/program-vdd-walrus-rslhve
```

## Why the path matters

The SP1 guest ELF can contain dependency source paths. In this project, `strings` on the ELF shows paths such as:

```text
/home/justin/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sp1-zkvm-6.2.4/src/...
```

Therefore, matching dependency versions is not enough. For byte-for-byte matching with the Ubuntu reference, the container build must use:

- source mount path: `/home/justin/TrustDrop/TrustDrop`
- Cargo home path: `/home/justin/.cargo`
- crates.io registry/cache copied from the Ubuntu reference, not the Mac user Cargo mirror cache
- Ubuntu ignored lockfiles copied into the repo before building

Do not mount or reuse a Mac mini `~/.cargo/config.toml` that rewrites crates.io to a mirror. During debugging, a USTC mirror config changed the effective registry/cache path and produced mismatching ELFs.

## Build the runner image

On Apple Silicon, build and run this image as linux/amd64. Docker Desktop with Rosetta, Colima with Rosetta, or another amd64-capable Docker backend is required.

```sh
docker build --platform linux/amd64 \
  -t trustdrop/elf-repro-runner:ubuntu-amd64 \
  -f docker/sp1-elf-repro/Dockerfile .
```

Sanity check:

```sh
docker run --rm --platform linux/amd64 trustdrop/elf-repro-runner:ubuntu-amd64 bash -lc '
  cargo prove --version
  cargo --version
  rustc +succinct --version
'
```

Expected versions:

```text
cargo-prove sp1 (98a376e 2026-05-15T03:56:11.493352970Z)
cargo 1.95.0 (f2d3ce0bd 2026-03-21)
rustc 1.93.0-dev
```

## Required local inputs

Before building, the Mac mini must have the Ubuntu-aligned Cargo cache available under the Mac user Cargo directory:

```text
/Users/niuniu/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f
/Users/niuniu/.cargo/registry/index/index.crates.io-1949cf8c6b5b557f
/Users/niuniu/.cargo/git
```

The repository should contain the Ubuntu ignored lockfiles:

```text
Cargo.lock
guest/vss/Cargo.lock
guest/vdd/Cargo.lock
guest/fibo3/Cargo.lock
```

These lockfiles are intentionally ignored by the current repository rules, but they are required for this exact historical reproducibility target.

## Build VSS

```sh
cd /Users/niuniu/TrustDrop/TrustDrop
rm -rf guest/vss/target /tmp/out-vss-cargohome-justin

docker run --rm --platform linux/amd64 \
  -e RUSTUP_TOOLCHAIN=stable-x86_64-unknown-linux-gnu \
  -e LC_ALL=C -e LANG=C -e CARGO_NET_OFFLINE=true \
  -v /Users/niuniu/.cargo/registry:/host-cargo-registry:ro \
  -v /Users/niuniu/.cargo/git:/host-cargo-git:ro \
  -v /Users/niuniu/TrustDrop/TrustDrop:/home/justin/TrustDrop/TrustDrop \
  -w /home/justin/TrustDrop/TrustDrop/guest/vss \
  trustdrop/elf-repro-runner:ubuntu-amd64 bash -lc '
    set -eux
    mkdir -p /home/justin/.cargo
    cp -a /host-cargo-registry /home/justin/.cargo/registry
    cp -a /host-cargo-git /home/justin/.cargo/git
    export CARGO_HOME=/home/justin/.cargo
    cargo prove build --locked -p vss-program \
      --binaries vss-program \
      --elf-name vss-program \
      --output-directory /tmp/out-vss-cargohome-justin \
      --warning-level all
    sha256sum /tmp/out-vss-cargohome-justin/vss-program
  '
```

Verify the target path:

```sh
shasum -a 256 guest/vss/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/vss-program
```

Expected:

```text
bbb48442fbb8bfcfbdca83cb5ebb5ca798b3697d9e59abd827d1d8383641bb2e
```

## Build VDD

```sh
cd /Users/niuniu/TrustDrop/TrustDrop
rm -rf guest/vdd/target /tmp/out-vdd-cargohome-justin

docker run --rm --platform linux/amd64 \
  -e RUSTUP_TOOLCHAIN=stable-x86_64-unknown-linux-gnu \
  -e LC_ALL=C -e LANG=C -e CARGO_NET_OFFLINE=true \
  -v /Users/niuniu/.cargo/registry:/host-cargo-registry:ro \
  -v /Users/niuniu/.cargo/git:/host-cargo-git:ro \
  -v /Users/niuniu/TrustDrop/TrustDrop:/home/justin/TrustDrop/TrustDrop \
  -w /home/justin/TrustDrop/TrustDrop/guest/vdd \
  trustdrop/elf-repro-runner:ubuntu-amd64 bash -lc '
    set -eux
    mkdir -p /home/justin/.cargo
    cp -a /host-cargo-registry /home/justin/.cargo/registry
    cp -a /host-cargo-git /home/justin/.cargo/git
    export CARGO_HOME=/home/justin/.cargo
    cargo prove build --locked -p program-vdd-walrus-rslhve \
      --binaries program-vdd-walrus-rslhve \
      --elf-name program-vdd-walrus-rslhve \
      --output-directory /tmp/out-vdd-cargohome-justin \
      --warning-level all
    sha256sum /tmp/out-vdd-cargohome-justin/program-vdd-walrus-rslhve
  '
```

Verify the target path:

```sh
shasum -a 256 guest/vdd/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/program-vdd-walrus-rslhve
```

Expected:

```text
791680137fe92209b774830c7272bda8b7c0c8e53c73345be3e0e51cbc3e69df
```


## Build and run seller `drop-cli` in the container

The same amd64 Ubuntu runner can build the seller-side CLI, with these extra notes:

- `libprotobuf-dev` must be installed in the image. `protobuf-compiler` alone is not enough for `sp1-prover-types`.
- If `sp1-core-executor-runner` fails to find `sp1-core-executor-runner-binary`, prebuild `sp1-core-executor-runner-binary` and set `SP1_CORE_RUNNER_OVERRIDE_BINARY` while building `drop-cli`.
- If the build stalls under `ethers-*` with a nested `cargo metadata`, the child Cargo process is usually waiting on the parent package-cache lock. Use a temporary Cargo wrapper so `cargo metadata` uses an independent `/home/justin/.cargo-metadata` cache.
- On the Mac mini, Docker containers do not automatically inherit the GUI VPN/system proxy. If external access needs a proxy, put the real HTTPS_PROXY, HTTP_PROXY, ALL_PROXY, and NO_PROXY values in local-only docker/seller/seller.env. Do not commit concrete proxy endpoints.

Minimal seller build flow:

```sh
cd /Users/niuniu/TrustDrop/TrustDrop

# Persistent Ubuntu-aligned Cargo home for container builds.
mkdir -p /Users/niuniu/.cargo-trustdrop-justin
cp -a /Users/niuniu/.cargo/registry /Users/niuniu/.cargo-trustdrop-justin/registry
cp -a /Users/niuniu/.cargo/git /Users/niuniu/.cargo-trustdrop-justin/git
rm -f /Users/niuniu/.cargo-trustdrop-justin/config /Users/niuniu/.cargo-trustdrop-justin/config.toml

# Inside the runner container:
runner_manifest=/home/justin/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sp1-core-executor-runner-binary-6.2.4/Cargo.toml
CARGO_TARGET_DIR=/home/justin/.cargo/sp1-native-bins cargo build --manifest-path "$runner_manifest"
runner_bin=/home/justin/.cargo/sp1-native-bins/debug/sp1-core-executor-runner-binary
SP1_CORE_RUNNER_OVERRIDE_BINARY="$runner_bin" cargo build -p drop-cli
target/debug/drop-cli --help
```

If the `ethers-*` metadata lock issue appears, create this wrapper inside the container and invoke it as Cargo:

```sh
cat >/tmp/trustdrop-cargo-wrapper <<'EOF'
#!/usr/bin/env bash
set -e
real_cargo=/root/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo
if [ "${1:-}" = "metadata" ]; then
  export CARGO_HOME=/home/justin/.cargo-metadata
  export CARGO_NET_OFFLINE=true
fi
export CARGO=/tmp/trustdrop-cargo-wrapper
exec "$real_cargo" "$@"
EOF
chmod +x /tmp/trustdrop-cargo-wrapper
```

Copy `/home/justin/.cargo/registry` and `/home/justin/.cargo/git` into `/home/justin/.cargo-metadata`, then run `CARGO=/tmp/trustdrop-cargo-wrapper /tmp/trustdrop-cargo-wrapper build -p drop-cli`.

Before starting the daemon, verify the container can reach the oracle worker through the Mac mini proxy:

```sh
docker run --rm --platform linux/amd64 \
  -e HTTPS_PROXY=${HTTPS_PROXY} \
  -e HTTP_PROXY=${HTTP_PROXY} \
  -e ALL_PROXY=${ALL_PROXY} \
  trustdrop/elf-repro-runner:ubuntu-amd64 \
  curl -sS https://trustdrop-oracle-worker.zhengxingao.workers.dev/health
```

Start seller daemon with the same proxy and mounted repo/Cargo home:

```sh
docker run --rm --platform linux/amd64 --name trustdrop-seller-daemon \
  -e DROP_CLI_ENV=drop-script/.env \
  -e SP1_CORE_RUNNER_OVERRIDE_BINARY=/home/justin/.cargo/sp1-native-bins/debug/sp1-core-executor-runner-binary \
  -e HTTPS_PROXY=${HTTPS_PROXY} \
  -e HTTP_PROXY=${HTTP_PROXY} \
  -e ALL_PROXY=${ALL_PROXY} \
  -v /Users/niuniu/.cargo-trustdrop-justin:/home/justin/.cargo \
  -v /Users/niuniu/TrustDrop/TrustDrop:/home/justin/TrustDrop/TrustDrop \
  -w /home/justin/TrustDrop/TrustDrop \
  trustdrop/elf-repro-runner:ubuntu-amd64 \
  target/debug/drop-cli daemon run
```

This image does not bundle a Walrus node or Walrus/Sui CLI. It only builds and runs the TrustDrop/SP1-side binaries; Walrus publisher/node settings still come from the migrated TrustDrop environment and external services.

## Troubleshooting

If the ELF hash does not match:

1. Confirm the commit is `53508829edd5c1b5c63b02b74d469e8d5c3be920`.
2. Confirm the ignored lockfiles are present and were copied from the Ubuntu reference.
3. Confirm the build uses `CARGO_HOME=/home/justin/.cargo` inside the container.
4. Confirm the repo is mounted at `/home/justin/TrustDrop/TrustDrop` inside the container.
5. Run `strings <elf> | grep -E "/tmp/cargo-home|/Users/niuniu|/home/justin/.cargo"` and ensure no Mac or temporary Cargo paths are embedded.
6. Confirm the container tool versions match the reference versions above.
7. Do not change Rust source or warning fixes while debugging reproducibility; warnings are part of the known-good build and should not be cleaned up in this path.
