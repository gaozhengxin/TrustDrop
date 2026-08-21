# 模块说明

## Rust Workspace

根 `Cargo.toml` 定义 workspace 成员：

- `drop-lib`
- `guest/*/script`
- `guest/*/program`
- `sdk`
- `app/gui/src-tauri`
- `storage`
- `walrus-core`
- `drop-script`

注意：`guest/vdd/program-vdd-*` 目录存在，但当前根 workspace glob 只包含 `guest/*/program`，因此这些 VDD program 不是根 workspace 成员。

## `drop-lib`

通用核心库，支持 host 和 guest 场景。

- `chacha8.rs`: ChaCha8 加解密、nonce 派生。
- `kdf.rs`: 交易密钥派生。
- `ecies.rs` / `elgamal.rs`: 公钥加密相关工具。
- `cid.rs`: IPFS CID 计算。
- `walrus_address.rs`: Walrus blob id 计算。
- `merkle.rs`: 数据分块 Merkle tree。
- `poseidon.rs`: Poseidon hash/encrypt/decrypt。
- `rslh_ve.rs`: Walrus/RSLH-VE 抽样同态验证逻辑。
- `common.rs`: ZK public output 解码辅助。

## `storage`

统一存储接口和两个 provider 实现。

- `StorageNetwork`: 定义 `upload_blob`、`download_blob`、`get_status`、`upload_file`、`download_file`。
- `WalrusClient`: 调用 publisher/aggregator API，Blockberry 查询状态。
- `FilecoinClient`: 本地 IPFS add 后调用 Lighthouse pin，并查询 Lighthouse 状态。
- CLI:
  - `cargo run --bin walrus -- upload|download|status ...`
  - `cargo run --bin filecoin -- upload|download|status ...`

## `sdk`

SDK 当前是薄封装：

- `abi.rs`: 由合约 ABI 生成或整理的 Rust 绑定。
- `chacha8.rs`: SDK 层加解密包装。
- `proof.rs`: VSS/VDD proof 入口占位。
- `walrus.rs`: `compute_rs_id` 与幂等上传辅助。

## `drop-script`

端到端演示主程序，是理解项目最重要的入口。

关键职责：

- 读取环境变量和链配置。
- 构造卖家/买家 signer。
- 上传密文数据。
- 创建 channel、挂牌、购买。
- 调用 SP1 prover 生成 VSS/VDD proof。
- 调用合约履约、等待 Oracle、结算。
- 下载并恢复文件。

## `contracts`

核心合约：

- `ExchangeHub.sol`: channel clone 工厂、注册 channel、转发事件。
- `ExchangeChannel.sol`: 挂牌、购买、履约、结算、退款。
- `VSS.sol`: audience 注册、数据密钥承诺、密钥分享 proof 验证、privy bitmap。
- `VDD.sol`: 数据承诺登记、VDD proof 验证、Oracle 可用性状态。
- `oracle/*`: hybrid OracleProxy、centralized Worker report path、CRE-compatible report path，以及历史 Chainlink Functions consumer 参考实现。

测试位于 `contracts/test/`，包括正向流程、安全路径、负向路径、bitmap、proxy 隔离等测试。

## `guest`

- `guest/vss`: VSS SP1 guest，证明数据密钥封装正确。
- `guest/vdd`: VDD SP1 guest，包含 Filecoin、Walrus、Walrus RSLH-VE 多个变体。
- `guest/fibo3`: SP1 模板/示例工程。

各 guest 子目录仍保留 SP1 模板 README，实际业务逻辑应优先看 `program*/src/main.rs` 和 `script/src/bin/*`。

## `walrus-core`

本地 no_std Walrus core 代码，提供 blob id、encoding、metadata、Merkle 等基础能力。`drop-lib` 通过 path dependency 使用它。

## `app/gui`

Tauri 2 + Vite + TypeScript。当前前端和 Rust command 仍是默认 greet 示例，还没有接入 Maenad 交易流程。
