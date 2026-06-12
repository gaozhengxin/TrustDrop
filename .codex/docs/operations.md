# 运行与环境

## 基础要求

- Rust toolchain: 根目录有 `rust-toolchain.toml`。
- Foundry: 编译和测试 `contracts/`。
- SP1 toolchain: 构建 guest ELF、生成本地或网络 proof。
- Node.js / pnpm 或 npm: 运行 `app/gui`。
- Walrus daemon 或可访问的 Walrus publisher/aggregator。
- 可用 EVM RPC 与测试网账户私钥。

## Rust

读取 workspace 元数据：

```sh
cargo metadata --no-deps --format-version 1
```

当前执行该命令能成功读取 workspace，但会提示：

```text
warning: patch for the non root package will be ignored
package: guest/vss/script/Cargo.toml
workspace: Cargo.toml
```

如果该 patch 是必须的，应迁移到根 `Cargo.toml` 的 workspace 层配置。

## 端到端脚本

入口：

```sh
cargo run -p drop-script
```

运行前请先阅读 [Drop Script 端到端流程](./drop-script.md)，确认合约地址、verifier VK、Walrus、Oracle 和 SP1 network 都是同一套环境。

关键环境变量和依赖：

- `SELLER_KEY`: 卖家私钥。
- `BUYER_KEY`: 买家私钥。
- `SP1_PRIVATE_KEY`: SP1 prover network 私钥，脚本会写入 `NETWORK_PRIVATE_KEY`。
- `Mo.mp4`: 脚本默认读取的输入资产文件。
- Walrus daemon: 默认 endpoint 是 `http://localhost:31415`。
- Arbitrum Sepolia RPC: 当前硬编码为 `https://sepolia-rollup.arbitrum.io/rpc`。

当前脚本中合约地址、链 ID、输入输出文件名均硬编码在 `drop-script/src/main.rs`，正式化前建议迁移到 `.env` 或配置文件。

编译相关命令：

```sh
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p drop-script
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p vss-script --bin vss
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p vdd-script --bin main_walrus_rslhve
cargo test -p drop-lib
```

## Walrus

参考 `storage/README.md`：

```sh
walrus daemon --sub-wallets-dir ~/.sui/sui_config --n-clients 1
```

上传：

```sh
cargo run --bin walrus -- upload --input ./testdata.txt --epoch 6
```

下载：

```sh
cargo run --bin walrus -- download --blob <blob_id> --output ./out
```

查询状态：

```sh
cargo run --bin walrus -- status --blob <blob_id>
```

## Filecoin / Lighthouse

上传：

```sh
cargo run --bin filecoin -- upload --input ./testdata.txt
```

下载：

```sh
cargo run --bin filecoin -- download --cid <cid> --output ./out
```

查询状态：

```sh
cargo run --bin filecoin -- status --cid <cid>
```

## 合约

在 `contracts/` 目录下：

```sh
forge build
forge test
forge fmt
```

部署脚本入口：

```sh
forge script script/DeployMain.s.sol --rpc-url <rpc_url> --private-key <private_key>
```

## GUI

在 `app/gui/` 目录下：

```sh
npm install
npm run dev
npm run tauri dev
```

当前 GUI 未接入协议，仅可作为后续产品界面的起点。
