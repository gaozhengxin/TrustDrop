# TrustDrop seller Docker runtime

This directory runs the seller-side stack on the Mac mini with Walrus inside Docker.

## Layout

The compose stack has two containers:

- `walrus-publisher`: native `linux/arm64` Walrus publisher sidecar, listening on `0.0.0.0:31415` inside the Docker network.
- `seller-daemon`: `linux/amd64` TrustDrop `drop-cli daemon`, using the already-built `target/debug/drop-cli` and connecting to `http://walrus-publisher:31415`.

This split is intentional. The Ubuntu `linux/amd64` Walrus binary can crash with `Illegal instruction` under Docker/Rosetta on Apple Silicon, while `drop-cli` currently uses the amd64 SP1/container build path.

## Host wallet/config mounts

The Sui wallet/config directory is supplied by the host and mounted into the Walrus container as `/home/justin/.sui`.

Default host value:

```text
TRUSTDROP_SUI_DIR=~/.sui
```

The helper script expands this to an absolute path before invoking Docker Compose. The directory must contain:

```text
sui_config/client.yaml
sui_config/sui.keystore
sui_config/sui_0.keystore ... sui_config/sui_7.keystore
```

On the Mac mini, these files were migrated from the Ubuntu reference environment and are required for Walrus to find managed addresses.

Walrus client config is mounted read-only from:

```text
TRUSTDROP_WALRUS_DIR=~/walrus
```

The expected config file is:

```text
~/walrus/client_config.yaml
```

## Local binary artifacts

The Docker build needs local binary artifacts under `docker/seller/bin/`; this directory is intentionally gitignored.

Current Mac mini artifacts:

```text
docker/seller/bin/linux-arm64/walrus   # official MystenLabs/walrus mainnet v1.53.0 ubuntu-aarch64
docker/seller/bin/linux-amd64/walrus   # Ubuntu reference copy; kept for debugging, not used by compose
docker/seller/bin/linux-amd64/sui      # Ubuntu reference copy; kept for debugging, not used by compose
```

Do not commit these binaries.


## Persistent seller state

The seller `drop-cli` state is persisted on the host via:

```text
TRUSTDROP_STATE_DIR=~/.trustdrop
```

Compose mounts it as `/root/.trustdrop` in every seller CLI container. This is required because host-file commands such as `phase prepare ~/Desktop/file.dat` run in one-off containers, while later commands and the long-running daemon must see the same sale state.

## Run

From the Mac mini:

```sh
cd /Users/niuniu/TrustDrop/TrustDrop/docker/seller
cp seller.env.example seller.env  # optional; edit paths if not using defaults
./run-seller-daemon.sh
```

The helper uses Docker Desktop's CLI path automatically when `docker` is not on the non-interactive shell PATH.


## Host-side `drop-cli` operations

Use `drop-cli-host.sh` for user-facing commands from the Mac mini host. It supports normal `drop-cli` commands and also accepts host file paths for file-taking commands.

```sh
cd /Users/niuniu/TrustDrop/TrustDrop
./docker/seller/drop-cli-host.sh doctor
./docker/seller/drop-cli-host.sh phase prepare ~/Desktop/demo.mp4
./docker/seller/drop-cli-host.sh phase publish <sale-id>
```

See `HOST_OPERATIONS.md` for the full command map.

## Checks

```sh
docker ps --filter name=trustdrop

docker exec trustdrop-seller-daemon bash -lc '
  curl -sS -o /tmp/walrus-api -w "seller_to_walrus=%{http_code} bytes=%{size_download}\n"     http://walrus-publisher:31415/v1/api
  env | grep -E "^WALRUS_(LOCAL_ENDPOINT|PUBLISHER_URL)="
'
```

Expected current result:

```text
seller_to_walrus=200
WALRUS_PUBLISHER_URL=http://walrus-publisher:31415
WALRUS_LOCAL_ENDPOINT=http://walrus-publisher:31415
```
