# Drop SDK 用户文档

## 定位

`drop-sdk` 是 TrustDrop 的 Rust SDK crate，目录是 `sdk/`。

当前它承担的是可复用客户端能力：

- 合约 ABI 绑定。
- ChaCha8 加密/解密工具。
- Walrus blob id 计算与幂等上传辅助。
- 证明 helper 的早期占位接口。

当前它还不是完整 seller CLI。完整端到端流程仍由 `drop-script` 编排。0008 的目标是逐步把 `drop-script` 中可复用的能力拆到 `drop-sdk`，最后让 CLI 调用 SDK，而不是在 CLI 中堆业务细节。

## 包名和导入

Cargo package name：

```toml
drop-sdk = { path = "../sdk" }
```

Rust crate import path：

```rust
use drop_sdk::chacha8::{chacha8_decrypt, chacha8_encrypt};
use drop_sdk::walrus::{compute_rs_id, upload_data_idempotent};
use drop_sdk::abi::exchange_channel_contract as channel_abi;
use drop_sdk::abi::exchange_hub_contract as hub_abi;
```

历史名称 `maenad-sdk` 已改为 `drop-sdk`。

## 当前模块

### `drop_sdk::abi`

由 `ethers::abigen!` 生成的合约绑定。

主要导出：

- `exchange_hub_contract`
- `exchange_channel_contract`
- `ExchangeChannelCreatedFilter`
- `DataKeySharedFilter`

用途：

- 创建 Channel。
- 调用 `listFile`、`purchase`、`fulfill`、`settle`。
- 解析 Hub/Channel 事件。

### `drop_sdk::chacha8`

导出：

```rust
pub fn chacha8_encrypt(
    data: &[u8],
    key: &[u8; 32],
    nonce: &[u8; 12],
    counter: u32,
) -> anyhow::Result<Vec<u8>>;

pub fn chacha8_decrypt(
    data: &[u8],
    key: &[u8; 32],
    nonce: &[u8; 12],
    counter: u32,
) -> anyhow::Result<Vec<u8>>;
```

说明：

- ChaCha8 是流密码，加密和解密使用同一个 keystream。
- `chacha8_decrypt` 当前直接调用 `chacha8_encrypt`。

### `drop_sdk::walrus`

导出：

```rust
pub fn compute_rs_id(data: &[u8]) -> anyhow::Result<[u8; 32]>;

pub async fn upload_data_idempotent(
    walrus: &storage::WalrusClient,
    data: Vec<u8>,
) -> anyhow::Result<String>;
```

说明：

- `compute_rs_id` 使用 `drop-lib` 中的 Walrus blob id 算法。
- `upload_data_idempotent` 先计算目标 blob id，再尝试查询 Walrus 状态；若已存在则直接返回，否则上传。
- SDK 只使用已有 Walrus endpoint，不负责安装或维护 Walrus publisher 节点。

### `drop_sdk::proof`

当前是早期占位接口：

```rust
pub async fn run_vss_proof(...) -> anyhow::Result<(Bytes, Bytes)>;
pub async fn run_vdd_proof(...) -> anyhow::Result<(Bytes, Bytes)>;
```

重要限制：

- 不要把该模块用于生产证明。
- 当前真实 SP1 Prove Network 证明逻辑仍在 `drop-script/src/main.rs`。
- 后续 0008 会把证明 orchestration 正式拆入 SDK，并保留“默认不本地 proving”的规则。

## 最小使用示例

### 计算并上传 Walrus blob

```rust
use drop_sdk::walrus::{compute_rs_id, upload_data_idempotent};
use storage::{WalrusClient, WalrusConfig};

async fn upload_example() -> anyhow::Result<()> {
    let endpoint = "http://localhost:31415".to_string();
    let walrus = WalrusClient::new(WalrusConfig {
        aggregator_url: endpoint.clone(),
        publisher_url: endpoint,
        api_key: "".into(),
        blockberry_base: "".into(),
        send_object_to: None,
    });

    let data = b"hello trustdrop".to_vec();
    let rs_id = compute_rs_id(&data)?;
    let blob_id = upload_data_idempotent(&walrus, data).await?;

    println!("rs_id=0x{}", hex::encode(rs_id));
    println!("blob_id={}", blob_id);
    Ok(())
}
```

### 调用合约绑定

```rust
use drop_sdk::abi::exchange_hub_contract as hub_abi;
use ethers::prelude::*;
use std::sync::Arc;

async fn create_channel(
    provider: Provider<Http>,
    wallet: LocalWallet,
    hub: Address,
    owner_pubkey: Vec<u8>,
) -> anyhow::Result<()> {
    let client = Arc::new(SignerMiddleware::new(provider, wallet));
    let hub_contract = hub_abi::ExchangeHubContract::new(hub, client);

    let pubkey = hub_abi::Pubkey {
        data: owner_pubkey.into(),
    };
    let pending = hub_contract.create_exchange_channel(pubkey).send().await?;
    let receipt = pending.await?;

    println!("create channel receipt={:?}", receipt);
    Ok(())
}
```

## drop-script 当前如何使用 SDK

`drop-script` 当前使用：

- `drop_sdk::abi`
- `drop_sdk::chacha8`
- `drop_sdk::walrus`

但仍由 `drop-script` 自己负责：

- `.env` 读取。
- 阶段编排。
- SP1 Prove Network 证明请求。
- VSS/VDD public values 校验。
- Oracle Worker 触发。
- `oracleSuccessUntil` 轮询。
- `settle` 和 recovery。

0008 后续要继续把这些职责拆入 SDK，但每一步都要保持 `drop-script` 主流程行为不变。

## 编译验证

推荐使用：

```sh
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p drop-sdk
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p drop-script
```

不要用完整 `cargo run -p drop-script` 当作 SDK 编译检查，因为完整脚本会进入 Walrus、链上交易和 SP1 Prove Network 业务流程。

## Seller CLI 方向

未来 seller CLI 应该调用 `drop-sdk`，而不是直接复制 `drop-script` 内部逻辑。

预期分层：

- CLI：读取用户配置、打印进度、处理命令参数。
- SDK：提供合约、存储、证明、oracle、workflow 能力。
- Walrus publisher setup skill：指导 agent 帮 seller 准备 publisher endpoint。

SDK 不应该承诺 Walrus 节点的具体安装命令、Docker Compose 文件或版本。SDK 只接受可用 endpoint。
