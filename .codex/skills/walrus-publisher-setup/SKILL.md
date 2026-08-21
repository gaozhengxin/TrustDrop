---
name: walrus-publisher-setup
description: Use when setting up, repairing, or diagnosing a TrustDrop seller Walrus mainnet publisher node, especially `/home/justin/walrus/start.sh`, local endpoint `127.0.0.1:31415`, Sui/WAL balances, sub-wallet pools, and Walrus client version compatibility.
---

# Walrus Publisher Setup

Use this skill before changing or restarting a seller Walrus publisher node.

## Safety Rules

- Do not upload blobs, run `drop-cli phase publish`, or send Sui transactions during diagnosis unless the user explicitly approves.
- Do not replace or discard an existing `--sub-wallets-dir` without first checking the contained wallet addresses and balances.
- Do not assume the main Sui wallet balance is the only relevant balance. Publisher mode can use sub-wallets.
- Do not run multiple publisher/proof/build tasks in parallel.
- If a command needs non-sandbox network access, rerun only the same readonly check with approval.

## Current TrustDrop Publisher Layout

Working directory:

```sh
/home/justin/walrus
```

Expected local endpoint:

```sh
http://127.0.0.1:31415
```

Main wallet:

```sh
/home/justin/.sui/sui_config/client.yaml
```

Known funded sub-wallet pool:

```sh
/home/justin/.sui/sui_config
```

Current recommended start script shape:

```sh
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
WALRUS_N_CLIENTS="${WALRUS_N_CLIENTS:-1}"
exec walrus daemon \
  --config "${SCRIPT_DIR}/client_config.yaml" \
  --wallet "${HOME}/.sui/sui_config/client.yaml" \
  --bind-address "127.0.0.1:31415" \
  --max-body-size 1048576000 \
  --sub-wallets-dir "${HOME}/.sui/sui_config" \
  --n-clients "${WALRUS_N_CLIENTS}" \
  --publisher-max-concurrent-requests "${WALRUS_N_CLIENTS}"
```

Why default to `WALRUS_N_CLIENTS=1`:

- The existing sub-wallet pool contains funded wallets.
- Initializing all 8 clients can hang or take a long time in this environment.
- Single-client mode is enough for local TrustDrop development and avoids unnecessary concurrent wallet/RPC pressure.
- Operators can explicitly run `WALRUS_N_CLIENTS=8 /home/justin/walrus/start.sh` if they need concurrency and accept the risk.

## Version Check

Always check both Sui and Walrus versions:

```sh
sui --version
walrus --version
```

If Walrus upload succeeds on Sui but the daemon returns an internal 500 such as:

```text
client internal error: no object changes in transaction response
```

then suspect Walrus client/RPC response compatibility first. Confirm the transaction digest readonly with:

```sh
sui client tx-block <digest> --json
```

If the Sui transaction status is `success` and contains `BlobRegistered` plus `objectChanges`, the blob was registered and the daemon failed while parsing/reporting the response.

## Official Mainnet Client Config

Use the current official mainnet config format rather than an old multi-context config when possible:

```yaml
system_object: 0x2134d52768ea07e8c43570ef975eb3e4c27a39fa6396bef985b5abc58d03ddd2
staking_object: 0x10b9d30c28448939ce6c4d6c6e0ffce4a7f8a4ada8248bdad09ef8b70e4a3904
n_shards: 1000
max_epochs_ahead: 53
rpc_urls:
  - https://fullnode.mainnet.sui.io:443
```

Keep it at:

```sh
/home/justin/walrus/client_config.yaml
```

Keep backups before replacing config or binary:

```sh
/home/justin/walrus/backups
```

## Readonly Checks

Check configured addresses:

```sh
rg 'active_address' /home/justin/.sui/sui_config/client.yaml /home/justin/.sui/sui_config/sui_client_*.yaml
```

Check main wallet balance:

```sh
sui client balance 0x812d376d814ad01c75e0e519c43b6349eb1bd8180f48868da80e536b4787ecc3
```

Check a sub-wallet balance:

```sh
sui client balance <sub_wallet_address>
```

Check whether a daemon is already running:

```sh
ps -eo pid,ppid,pcpu,pmem,etime,cmd | rg 'walrus daemon'
```

Check local API after user-approved start:

```sh
curl -sS http://127.0.0.1:31415/v1/api >/tmp/walrus-api-check.html && wc -c /tmp/walrus-api-check.html
```

Sandboxed commands may fail to reach `127.0.0.1:31415`; in that case, repeat the same readonly `curl` with approval.

## Minimal Blob Read Check

Convert decimal blob id if needed:

```sh
walrus convert-blob-id <decimal_blob_id> --config /home/justin/walrus/client_config.yaml --wallet /home/justin/.sui/sui_config/client.yaml
```

Read a small range only:

```sh
curl -sS --range 0-15 http://127.0.0.1:31415/v1/blobs/<base64url_blob_id> | xxd -p -l 16
```

This checks aggregator readability without uploading or sending transactions.

## Upload Test Boundary

Only run an upload test after explicit approval. Prefer using the actual TrustDrop `drop-cli phase publish` state rather than ad hoc scripts, because TrustDrop needs the returned blob id, state file, sale metadata, and later VDD proof inputs to stay aligned.

If a Walrus upload returns an error but logs a Sui digest, inspect the digest before retrying. A retry may create another blob registration and consume WAL/SUI.

## Lessons From 2026-07-07

- The funded sub-wallet pool in `~/.sui/sui_config` was intentional and must not be discarded.
- The previous binary `walrus 1.48.1-9c5590a81e29` returned an internal 500 after a successful Sui transaction.
- Upgrading to official mainnet `walrus 1.50.0-dac31b8cb87c`, using the official mainnet `client_config.yaml`, and starting with `WALRUS_N_CLIENTS=1` made the local daemon usable.
- The repaired node successfully returned `/v1/api`, read an existing blob, and allowed `drop-cli phase publish` to upload/list the Apollo asset.
