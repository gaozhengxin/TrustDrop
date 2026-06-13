# Drop Script 调试物料与推进计划

日期：2026-06-12

目标：先不深入密码学细节，从结构层面确认 `drop-script` 的链路闭合，保证 Walrus、SP1 Prover Network、Arbitrum Sepolia 合约和 subgraph 可以围绕同一套部署调试。

## 当前结构闭环

`drop-script` 端到端链路：

1. 本地读取 `drop-script/Mo.mp4`。
2. 用固定演示数据密钥加密资产。
3. 上传密文到本地 Walrus publisher。
4. 通过 ExchangeHub 创建 ExchangeChannel。
5. Channel `listFile` 上架销售。
6. Seller 提交 data key commitment。
7. Buyer 调用 `purchase` 并锁定 ETH。
8. Seller 生成 VSS/VDD proof，并先用 verifier 合约做静态调用模拟。
9. Seller 调用 `fulfill`，Channel 内部执行 VSS/VDD 验证并触发 OracleProxy。
10. 脚本轮询 Channel `oracleSuccessUntil(cCipher)`。
11. Oracle 成功后调用 `settle`。
12. Buyer 从 `DataKeyShared` 事件恢复数据密钥，下载 Walrus 密文并恢复文件。

合约链路：

- `ExchangeHub`: 创建 channel clone，并聚合销售、购买、结算、退款事件。
- `ExchangeChannelImplementation`: 管理 sale、purchase、fulfill、settle、refund。
- `VSS`: 管理 audience、data key commitment、VSS proof 验证、`DataKeyShared`。
- `VDD`: 管理 VDD proof、Oracle 请求、`oracleSuccessUntil`。
- `OracleProxy` + `WalrusFunctionsConsumer`: 负责 Chainlink Functions 回调。

subgraph 链路：

- `ExchangeHub` 为主数据源，索引 channel 创建、sale、purchase、settle、refund。
- 每个新建 channel 通过 `ExchangeChannelTemplate` 动态索引 `Joined`、`DataKeyCommitmentUpdated`、`DataKeyShared`、`VDDProofSubmitted`、`OracleRequestSkipped`。

## 已确认物料

### Walrus publisher

目录：`/home/justin/walrus`

启动脚本：

```sh
/home/justin/walrus/start.sh
```

脚本当前使用：

- config: `/home/justin/walrus/client.yaml`
- context: `mainnet`
- wallet: `${HOME}/.sui/sui_config/client.yaml`
- endpoint: `http://localhost:31415`

### drop-script 环境

文件：`drop-script/.env`

必须包含：

```sh
SELLER_KEY=...
BUYER_KEY=...
SP1_PRIVATE_KEY=...
ARBITRUM_SEPOLIA_RPC=https://sepolia-rollup.arbitrum.io/rpc
WALRUS_LOCAL_ENDPOINT=http://localhost:31415
HUB_ADDRESS=0x2e506eF3F3cE222F276ddA64Df239CEF92683a78
VSS_VERIFIER_ADDRESS=0x5e80ed679fb9f4050a5c7ede5ccbe39178f142a2
VDD_VERIFIER_ADDRESS=0x154D59Ed30B7784B5c9324b32b9ec5d6c8DE4071
DROP_ORACLE_TIMEOUT_SECS=1800
```

本地真实 `.env` 已给三个私钥上方补充对应地址注释。该文件不进 git。

### contracts 环境

文件：`contracts/.env`

后续部署合约统一使用这里的私钥和 Chainlink / verifier 配置。

当前本地最新 Foundry broadcast：

| 合约 | 地址 |
| --- | --- |
| ExchangeHub | `0x2e506eF3F3cE222F276ddA64Df239CEF92683a78` |
| OracleProxy | `0xdd004ceedb0b34adb4feb886e6bef96e947ed59b` |
| WalrusFunctionsConsumer | `0x5b56a45fff2f46c997a434ef2fca973e2a1f1c6b` |
| ExchangeChannelImplementation | `0x4e0aac9301f04aabb98770bdb043dd321f978feb` |
| VSS verifier | `0x5e80ed679fb9f4050a5c7ede5ccbe39178f142a2` |
| VDD verifier | `0x154D59Ed30B7784B5c9324b32b9ec5d6c8DE4071` |

注意：`contracts/deployed.md` 里有旧地址记录，调试时以 `contracts/broadcast/DeployMain.s.sol/421614/run-latest.json` 和实际 `.env` 为准。

### subgraph 环境

Studio 项目：

```text
https://thegraph.com/studio/subgraph/test-arbitrum-store/
```

文件：`subgraph/.env`

必须包含：

```sh
SUBGRAPH_SLUG=...
DEPLOY_KEY=...
```

当前 subgraph manifest：

- network: `arbitrum-sepolia`
- Hub: `0x2e506eF3F3cE222F276ddA64Df239CEF92683a78`
- startBlock: `256170177`

## 标准验证命令

### Rust / drop-script

```sh
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p drop-script
```

### 合约

```sh
forge build
forge test
```

要求：

- `contracts/lib/forge-std` 已初始化。
- `contracts/lib/openzeppelin-contracts` 已初始化并固定到兼容当前 Foundry 的版本。
- `contracts/foundry.toml` 包含 `forge-std` 和 OpenZeppelin remapping。

### subgraph

```sh
pnpm --dir subgraph install
pnpm --dir subgraph codegen
pnpm --dir subgraph build
pnpm --dir subgraph deploy:studio
```

部署前检查：

- `subgraph/.env` 是 `KEY=value` 格式。
- `subgraph/subgraph.yaml` 的 Hub 地址和 startBlock 对应当前部署。
- 若合约事件签名变化，先更新 `subgraph/abis/*.json` 和 mappings。

## 本轮结构检查结果

已通过：

- `cargo check -p drop-script`
- `pnpm --dir subgraph codegen`
- `pnpm --dir subgraph build`
- `forge build`
- `forge test`，15 个测试通过

已修正：

- `drop-script` 的 RPC、Walrus endpoint、Hub、VSS verifier、VDD verifier 改为从环境变量读取，常量只作为默认回退。
- `drop-script` 配置检查改为检查同一套环境变量地址，并补充 Hub code 检查。
- `drop-script/.env` 已补私钥地址注释和合约地址配置。
- `subgraph/.env` 规范为 shell 可加载的 `KEY=value`。
- `subgraph` 目录已重建为可 codegen/build 的项目。
- `contracts/foundry.toml` 已补 `forge-std` 和 OpenZeppelin remapping。
- OpenZeppelin submodule 固定到 `v5.0.2`，避免当前 Foundry 不支持 `evm_version = prague`。

## 结构风险与待办

1. `drop-script` 仍使用演示固定密钥：
   - seller VSS secret: `[0x11; 32]`
   - asset encryption key: `[0x22; 32]`
   后续真实流程需要改成可配置或由安全密钥管理生成。

2. `fulfill` 后依赖 Chainlink Functions 异步回调：
   - `fulfill` 成功不代表能 `settle`。
   - 必须确认 OracleProxy 已绑定 consumer，consumer 已配置 subscription，subscription 余额充足。

3. `contracts/deployed.md` 和最新 broadcast 不一致：
   - 后续每次部署后必须同步更新部署文档、`drop-script/.env`、`subgraph/subgraph.yaml`。

4. subgraph 目前索引核心事件，不索引链上状态读模型：
   - 已覆盖销售、购买、结算、退款、密钥分享、VDD proof、oracle skipped。
   - 如果要展示 `oracleSuccessUntil`，需要通过事件补充或增加链上 call handler 方案；当前合约没有成功回调事件。

5. `stage_4_recovery` 仍按 channel 上最后一条 `DataKeyShared` 找事件：
   - 单次调试可用。
   - 多订单并发时应从本次 `fulfill` receipt 精确解析。

## 下一轮调试顺序

1. 启动 Walrus publisher：

```sh
/home/justin/walrus/start.sh
```

2. 验证本地依赖：

```sh
pnpm --dir subgraph build
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p drop-script
forge test
```

3. 部署或确认合约：

- 使用 `contracts/.env`。
- 如果重新部署，更新：
  - `contracts/deployed.md`
  - `drop-script/.env`
  - `subgraph/subgraph.yaml`

4. 部署 subgraph：

```sh
pnpm --dir subgraph deploy:studio
```

5. 运行 drop-script 前置检查：

- Hub/VSS/VDD 地址必须有 code。
- Seller/Buyer/SP1 地址必须有足够 ETH。
- SP1 私钥必须有 Prover Network 权限和额度。
- Walrus endpoint 必须连通。

6. 跑端到端脚本：

```sh
cd drop-script
PROTOC=/tmp/protoc-25.3/bin/protoc cargo run -p drop-script
```

7. 如果卡住，按阶段定位：

- Listing 失败：查 Walrus 上传和 Hub/channel 创建。
- Purchase 失败：查 saleId、dataVersion、price、buyer balance。
- VSS/VDD 模拟失败：查 guest ELF、verifier VK、binding hash。
- Fulfill 成功但 settle 卡住：查 OracleProxy、consumer、Chainlink Functions subscription 和 `oracleSuccessUntil`。
- Recovery 失败：查 `DataKeyShared` 事件和 Walrus 下载。
