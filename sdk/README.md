# Drop SDK

`drop-sdk` is the Rust SDK crate for reusable TrustDrop client-side helpers.
It is currently used by `drop-script` and is being prepared as the shared layer
for future seller CLI workflows.

## Current Modules

- `drop_sdk::abi`
  - Ethers ABI bindings for `ExchangeHub` and `ExchangeChannel`.
  - Used by scripts and clients to call deployed TrustDrop contracts.
- `drop_sdk::chacha8`
  - ChaCha8 encrypt/decrypt helpers.
  - Used for asset encryption and recovery.
- `drop_sdk::walrus`
  - Walrus blob id calculation and idempotent upload helper.
  - Uses the `storage` crate `WalrusClient`.
- `drop_sdk::proof`
  - Experimental proof helper placeholders.
  - Do not use this module for production proof generation yet; the production
    SP1 Prove Network flow still lives in `drop-script`.

## Cargo Usage

From this workspace:

```toml
[dependencies]
drop-sdk = { path = "../sdk" }
```

Rust import path:

```rust
use drop_sdk::chacha8::{chacha8_decrypt, chacha8_encrypt};
use drop_sdk::walrus::{compute_rs_id, upload_data_idempotent};
use drop_sdk::abi::exchange_channel_contract as channel_abi;
use drop_sdk::abi::exchange_hub_contract as hub_abi;
```

## Walrus Helpers

```rust
use drop_sdk::walrus::{compute_rs_id, upload_data_idempotent};
use storage::{WalrusClient, WalrusConfig};

# async fn example() -> anyhow::Result<()> {
let cfg = WalrusConfig {
    aggregator_url: "http://localhost:31415".to_string(),
    publisher_url: "http://localhost:31415".to_string(),
    api_key: "".into(),
    blockberry_base: "".into(),
    send_object_to: None,
};
let walrus = WalrusClient::new(cfg);

let payload = b"hello trustdrop".to_vec();
let rs_id = compute_rs_id(&payload)?;
let blob_id = upload_data_idempotent(&walrus, payload).await?;
# Ok(())
# }
```

The SDK assumes a Walrus publisher/aggregator endpoint already exists. It does
not install or operate a Walrus node.

## ABI Bindings

```rust
use drop_sdk::abi::exchange_hub_contract as hub_abi;
use ethers::prelude::*;
use std::sync::Arc;

# async fn example(provider: Provider<Http>, signer: LocalWallet, hub: Address) -> anyhow::Result<()> {
let client = Arc::new(SignerMiddleware::new(provider, signer));
let hub_contract = hub_abi::ExchangeHubContract::new(hub, client);
let owner_pubkey = hub_abi::Pubkey { data: vec![1u8; 33].into() };
let pending = hub_contract.create_exchange_channel(owner_pubkey).send().await?;
# Ok(())
# }
```

## Build Check

Use `cargo check`, not a proof run, for SDK development:

```sh
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p drop-sdk
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p drop-script
```

Some SP1 dependencies use build scripts that may write outside the workspace.
If sandboxed checks fail with a read-only Cargo registry error, rerun in the
approved non-sandbox environment.

## Current Limitations

- The crate name has been renamed from `maenad-sdk` to `drop-sdk`.
- `drop-script` still owns the full end-to-end workflow orchestration.
- Production SP1 proof generation is not yet provided by `drop-sdk`.
- Seller-facing CLI commands and full live integration tests are planned for
  iteration 0008.
