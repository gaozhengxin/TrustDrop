# 迭代: drop-cli

## 日期

2026-06-28

## 背景

0008 已开始把 `drop-script` 中的可复用能力拆向 `drop-sdk`。但面向 seller 的最终交付形态不应是开发脚本，而应是一个稳定 CLI。

本轮目标是先明确 `drop-cli` 的功能边界、命令结构、配置方式、状态管理和测试策略。代码实现需要等设计确认后再推进。

当前用户决策：

- 暂不考虑正式环境。
- `drop-cli` 先在 Arbitrum Sepolia 上运行一段时间。
- Oracle 先固定使用中心化 Oracle Worker。
- CRE / Chainlink Oracle 分支不进入本轮 CLI 实施范围。

## 目标

- 设计 seller 使用的 `drop-cli`。
- 明确哪些功能进入 CLI，哪些能力应下沉到 `drop-sdk`。
- 明确配置、状态文件、命令、测试和验收标准。
- 保证未来重构后 `drop-script` 主流程功能不丢失。

## 范围

本轮文档覆盖：

- `drop-cli` 定位。
- seller 配置模型。
- 本地状态文件模型。
- 交易状态管理系统。
- CLI 命令设计。
- 原子命令与阶段聚合命令。
- `drop-sdk` 拆分要求。
- 测试策略和实施优先级。

本轮不默认实施：

- 不创建 CLI crate。
- 不改 `drop-script` 主流程。
- 不发链上交易。
- 不上传 Walrus blob。
- 不请求 SP1 证明。

## 实施方法

先完成设计文档，等待用户确认后再实施代码。

2026-06-29 用户已批准开始实施。实施范围先按原型版推进：

- 允许新增 `drop-cli` crate。
- 允许在 `drop-sdk` 增加 `config`、`oracle`、`state` 等基础模块。
- 第一版优先完成只读检查、状态管理和 Oracle Worker blob status 查询。
- 会发链上交易、上传 Walrus、提交 SP1 proof 的命令先以明确提示和状态门禁为主，逐步实现。

后续代码实施时应按以下顺序推进：

1. 先补 `drop-sdk` 中可复用模块。
2. 再创建 `drop-cli` 的 thin CLI 层。
3. 保持 `drop-script` 主流程行为不变。
4. 对每个 CLI 命令建立最小测试。
5. 最后再考虑聚合型 full-flow 命令。

## 设计笔记

### 定位

`drop-cli` 是面向数据卖家的命令行工具。它不承载协议核心逻辑，而是调用 `drop-sdk` 完成可复用能力，并把复杂流程包装成稳定、可诊断、可恢复的 seller 操作入口。

当前 `drop-script` 仍作为端到端 demo 和集成调试入口。后续重构目标是：

- `drop-sdk`: 提供合约、Walrus、Oracle Worker、证明和 workflow 能力。
- `drop-cli`: 读取配置、管理本地状态、展示进度、调用 SDK。
- `drop-script`: 保留为开发集成脚本，逐步减少业务细节。

### 设计原则

- 默认不隐藏关键风险。涉及链上交易、Walrus 上传、SP1 证明请求、Oracle Worker 触发时，CLI 必须明确打印即将执行的动作。
- 默认可恢复。每个阶段完成后都写入本地状态文件，失败后可以从最近成功阶段继续。
- 默认不暴露密钥。私钥、API key、worker token 不打印、不写入日志、不进入状态文件。
- 默认不本地 proving。开发和测试阶段证明请求走 SP1 Prove Network，除非用户显式选择其它 provider。
- Walrus publisher 节点由 seller 自己准备。CLI 只检查 endpoint，不负责发布官方 Docker Compose 或替用户长期运维节点。
- 外部服务失败时优先给出下一步诊断命令，而不是盲目重试。

### 配置

建议默认配置路径：

```text
~/.trustdrop/seller.toml
```

项目内开发配置可以继续使用：

```text
drop-script/.env
```

必要配置项：

- `chain.rpc_url`: Arbitrum Sepolia RPC。
- `chain.chain_id`: 当前为 `421614`。
- `seller.private_key`: seller 交易私钥。
- `buyer.public_key`: 用于测试或 demo 的 buyer public key，可选。
- `contracts.hub`: `ExchangeHub` 地址。
- `contracts.oracle_proxy`: Oracle proxy 地址。
- `contracts.vss_verifier`: VSS verifier 地址。
- `contracts.vdd_verifier`: VDD verifier 地址。
- `walrus.publisher_url`: seller 本地或远程 Walrus publisher endpoint。
- `walrus.aggregator_url`: Walrus aggregator endpoint。
- `oracle.worker_url`: 中心化 Oracle Worker URL。
- `oracle.worker_token`: 中心化 Oracle Worker token。
- `sp1.private_key`: SP1 Prove Network 私钥。

可选配置项：

- `subgraph.url`: 查询当前 listing/order 状态。
- `walrus.blockberry_base_url`: 只读 Walrus metadata API。
- `asset.default_epochs`: 默认 Walrus 保存 epoch 数。
- `workflow.state_dir`: 本地流程状态目录。
- `log.level`: 日志级别。

### 本地状态

CLI 应为每个 listing / sale 生成状态文件，避免失败后丢上下文。状态系统不是简单日志，而是 CLI 的核心产品能力：它必须持续跟踪每个阶段、每笔交易、每个外部请求的状态，并引导用户执行下一步命令。

建议路径：

```text
~/.trustdrop/state/<sale-id>.json
```

状态文件可记录：

- `sale_id`
- `channel_address`
- `input_asset_path`
- `original_asset_id`
- `encrypted_blob_id`
- `walrus_blob_id`
- `data_commitment`
- `data_version`
- `vss_proof_tx_or_fixture`
- `vdd_proof_tx_or_fixture`
- `purchase_tx_hash`
- `fulfill_tx_hash`
- `oracle_report_tx_hash`
- `settle_tx_hash`
- 每个阶段的 `started_at`、`finished_at`、`status`
- `next_actions`: CLI 建议的下一步命令列表。
- `last_error`: 最近失败原因和诊断建议。

不得记录：

- seller private key
- SP1 private key
- Oracle Worker token
- Walrus API key
- 明文 data key

### 交易状态管理

每笔链上交易必须有独立记录，而不是只保存 tx hash。

建议结构：

```json
{
  "id": "list_file_001",
  "kind": "list_file",
  "chainId": 421614,
  "txHash": "0x...",
  "status": "submitted",
  "requiredConfirmations": 1,
  "blockNumber": null,
  "receiptStatus": null,
  "createdAt": "2026-06-29T00:00:00Z",
  "updatedAt": "2026-06-29T00:00:00Z",
  "nextCommand": "drop-cli status <sale-id>"
}
```

交易状态枚举：

- `draft`: 已构造但未提交。
- `submitted`: 已提交 tx hash，等待 receipt。
- `confirmed`: receipt 成功且确认数满足要求。
- `reverted`: receipt 失败。
- `replaced`: nonce 被替换。
- `stale`: 长时间未确认，需要用户检查。
- `unknown`: RPC 当前无法确认状态。

外部请求状态也要纳入同一状态系统：

- Walrus 上传：`pending` / `certified` / `failed` / `expired`。
- SP1 Prove Network：`requested` / `proving` / `fulfilled` / `failed`。
- Oracle Worker：`requested` / `report_submitted` / `callback_confirmed` / `failed`。
- Subgraph 索引：`waiting` / `indexed` / `lagging` / `unavailable`。

状态刷新规则：

- 每次执行任意命令前，先读取本地状态。
- 对已有 tx hash 做只读 receipt 刷新。
- 对 Oracle / Walrus / SP1 / subgraph 做必要的只读刷新。
- 刷新后再决定是否允许当前命令继续。
- 如果状态不满足前置条件，CLI 必须停止并给出下一步建议。

### 状态查看与下一步引导

#### `drop-cli status <sale-id>`

展示一个 sale 的当前状态。

输出内容：

- 当前阶段。
- 每个阶段是否完成。
- 每笔 tx 的 hash、状态、确认数和失败原因。
- Walrus blob 是否 active / expired / not found。
- VSS/VDD proof 是否已生成和提交。
- Oracle 是否已 report，`oracleSuccessUntil` 是否有效。
- subgraph 是否已经索引关键事件。
- 建议下一步命令。

该命令只读，不发交易、不上传、不证明。

#### `drop-cli next <sale-id>`

只输出推荐的下一步命令。

例子：

```text
Next action:
  drop-cli proof vdd 0xSALE

Reason:
  VSS proof is submitted, encrypted blob is active, VDD proof is missing.
```

该命令适合给用户或 agent 做流程导航。

#### `drop-cli tx status <tx-hash>`

查询任意交易状态。

职责：

- 查询 receipt。
- 判断是否成功、reverted、pending 或 unknown。
- 如能关联到本地 sale 状态，则更新对应状态文件。

#### `drop-cli tx resume <sale-id>`

刷新状态并尝试从最近安全检查点恢复。

职责：

- 不重复发已确认交易。
- 不重复上传已 active 的 Walrus blob。
- 不重复提交已完成的 proof。
- 对 pending / unknown / failed 状态给出下一步建议。

### 命令设计

命令分两层：

- 原子命令：只做一个明确动作，便于调试和恢复。
- 阶段聚合命令：串起同一阶段的多个原子命令，但仍在发交易、上传、证明、Worker fulfill 前做确认。

#### `drop-cli init`

初始化本地配置。

职责：

- 创建 `~/.trustdrop/`。
- 生成配置模板。
- 检查必填项是否缺失。
- 不自动生成或覆盖私钥，除非后续明确设计 key management。

#### `drop-cli doctor`

只读环境检查。

检查内容：

- RPC 可访问，chain id 为 `421614`。
- seller 地址解析成功。
- seller ETH 余额足够支付测试交易。
- Hub / OracleProxy / verifier 地址存在代码。
- Hub 中配置的 OracleProxy / verifier 与配置一致。
- Oracle Worker `/health` 和 `/status` 正常。
- Walrus publisher / aggregator endpoint 可访问。
- Walrus blob status API 可用。
- SP1 Prove Network key 已配置，但不发证明请求。
- subgraph URL 可访问，可选。

不得执行：

- 不发链上交易。
- 不上传 Walrus blob。
- 不请求 SP1 proof。
- 不部署合约或 subgraph。

#### `drop-cli channel create`

创建或确认 seller 的 ExchangeChannel。

职责：

- 从配置读取 seller key 和 owner public key。
- 调用 Hub 创建 Channel。
- 解析 `ExchangeChannelCreated` 事件。
- 写入本地状态或本地 profile。

需要确认：

- 这是链上交易。

#### `drop-cli asset prepare <file>`

准备本地资产。

职责：

- 读取文件。
- 计算 `original_asset_id`。
- 生成 asset encryption key 和 nonce。
- 加密资产。
- 计算 `encrypted_blob_id`。
- 写入本地状态。

不得执行：

- 不上传 Walrus。
- 不发链上交易。
- 不请求证明。

#### `drop-cli asset upload <sale-id>`

上传密文资产到 Walrus。

职责：

- 从状态读取密文资产。
- 调用 Walrus publisher 上传。
- 保存 `walrus_blob_id`。
- 调用 Oracle Worker `/walrus/blob-status` 或 Blockberry 检查可见性。

需要确认：

- 这会消耗 Walrus 存储资源。

#### `drop-cli sale list <sale-id>`

上架数据。

职责：

- 构造 `dataCommitment`。
- 计算 `dataVersion = keccak256(dataCommitment)`。
- 调用 Channel `listFile` 或对应 listing 方法。
- 校验链上 event 和本地状态一致。

需要确认：

- 这是链上交易。

#### `drop-cli proof vss <sale-id>`

生成并提交 VSS 证明。

职责：

- 从状态读取 VSS 输入。
- 使用 SP1 Prove Network 生成证明。
- 校验 public values。
- 调用合约提交证明或分享 data key。
- 保存 proof metadata 和 tx hash。

规则：

- 默认不用本地 proving。
- 证明请求失败后停止，不自动重试。

#### `drop-cli proof vdd <sale-id>`

生成并提交 VDD 证明。

职责：

- 下载或读取 Walrus 密文。
- 构造 VDD 输入。
- 使用 SP1 Prove Network 生成证明。
- 校验 public values 中的 `c_origin`、`c_key`、`c_cipher`。
- 调用 `submitVDDProof`。
- 保存 tx hash。

规则：

- 默认不用本地 proving。
- 必须确认 verifier 地址和 VK 版本匹配。
- 证明失败后停止。

#### `drop-cli oracle check <sale-id>`

检查 Oracle/Walrus 可见性。

职责：

- 查询 `encrypted_blob_id` / `walrus_blob_id` 对应状态。
- 返回 `status`、`found`、`expired`、`endEpoch`、`expiresAt`。
- 读取链上 `oracleSuccessUntil(cCipher)`。

不得执行：

- 不触发 Worker fulfill。
- 不发链上交易。

#### `drop-cli oracle fulfill <tx-hash>`

触发中心化 Oracle Worker。

职责：

- 调用 Worker `/oracle/fulfill`。
- 传入 purchase / fulfill 交易 hash。
- 保存 `reportTxHash`。

需要确认：

- Worker 会用 relayer私钥发链上交易。

#### `drop-cli settle <sale-id>`

结算。

职责：

- 检查 VDD proof 已提交。
- 检查 Oracle 成功窗口有效。
- 检查 purchase 信息和 data version 一致。
- 调用 Channel settle。
- 保存 tx hash。

需要确认：

- 这是链上交易。

#### `drop-cli recover-test <sale-id>`

开发期验证买家恢复路径。

职责：

- 下载 Walrus 密文。
- 使用测试密钥或状态中的开发数据恢复文件。
- 对比原文哈希。

限制：

- 只用于开发和集成测试。
- 正式 seller CLI 不应要求 seller 持有 buyer 私钥。

### 阶段聚合命令

第一到第四阶段的每个功能都可以单独提供原子命令，但每个阶段也必须提供一个聚合命令。聚合命令用于降低 seller 操作复杂度；原子命令用于调试、恢复和高级控制。

#### `drop-cli phase prepare <file>`

阶段目标：完成本地准备，不触发外部消耗。

内部步骤：

- `drop-cli doctor --local`
- `drop-cli asset prepare <file>`
- 创建或更新本地 sale 状态。

允许自动执行：

- 读取文件。
- 本地加密。
- 本地 commitment / id 计算。
- 状态文件写入。

不得自动执行：

- Walrus 上传。
- 链上交易。
- SP1 证明请求。

#### `drop-cli phase publish <sale-id>`

阶段目标：把资产上传到 Walrus，并在链上发布 listing。

内部步骤：

- `drop-cli doctor --section walrus,contracts`
- `drop-cli asset upload <sale-id>`
- `drop-cli oracle check <sale-id>`
- `drop-cli channel create`，如果 seller 尚无 channel。
- `drop-cli sale list <sale-id>`
- `drop-cli status <sale-id>`

需要确认：

- Walrus 上传会消耗存储资源。
- Channel 创建和 listing 是链上交易。

状态要求：

- 若 Walrus blob 已 active，不重复上传。
- 若 Channel 已存在，不重复创建。
- 若 listing tx 已 confirmed，不重复 list。

#### `drop-cli phase prove <sale-id>`

阶段目标：完成 VSS/VDD 证明和链上提交。

内部步骤：

- `drop-cli proof vss <sale-id>`
- `drop-cli proof vdd <sale-id>`
- `drop-cli status <sale-id>`

需要确认：

- 会提交 SP1 Prove Network 请求。
- proof 生成后会发链上交易提交或使用证明。

状态要求：

- 已完成的 proof 不重复请求。
- Prove Network 失败后停止，要求用户检查错误并明确批准后才能重试。
- verifier 地址 / VK 不匹配时停止。

#### `drop-cli phase settle <sale-id>`

阶段目标：完成 Oracle fulfill 和结算。

内部步骤：

- `drop-cli oracle check <sale-id>`
- 如果缺少 Oracle report，则提示用户提供 fulfill tx hash 或从状态读取。
- `drop-cli oracle fulfill <tx-hash>`
- `drop-cli oracle check <sale-id>`
- `drop-cli settle <sale-id>`
- `drop-cli status <sale-id>`

需要确认：

- Oracle Worker 会用 relayer 发链上交易。
- settle 是 seller 链上交易。

状态要求：

- `oracleSuccessUntil(cCipher)` 有效后才允许 settle。
- report tx 已 confirmed 时不重复 fulfill。
- settle tx 已 confirmed 时不重复 settle。

#### `drop-cli phase verify <sale-id>`

阶段目标：开发期验证完整结果。

内部步骤：

- `drop-cli status <sale-id>`
- `drop-cli recover-test <sale-id>`
- 可选 subgraph 查询，确认事件已索引。

限制：

- 只用于开发和集成测试。
- 正式 seller 发行版中可以隐藏或标为 dev command。

### 推荐主流程

开发期 happy path：

```text
drop-cli doctor
drop-cli phase prepare <file>
drop-cli status <sale-id>
drop-cli phase publish <sale-id>
drop-cli status <sale-id>
drop-cli phase prove <sale-id>
drop-cli status <sale-id>
drop-cli phase settle <sale-id>
drop-cli phase verify <sale-id>
```

完整聚合命令可以作为最后阶段能力：

```text
drop-cli run full-flow <file>
```

但完整聚合命令必须复用阶段聚合命令，并仍然在每个会消耗资源或发交易的阶段前输出确认信息。

调试时仍可直接调用原子命令：

```text
drop-cli asset prepare <file>
drop-cli asset upload <sale-id>
drop-cli sale list <sale-id>
drop-cli proof vss <sale-id>
drop-cli proof vdd <sale-id>
drop-cli oracle fulfill <tx-hash>
drop-cli settle <sale-id>
```

### SDK 拆分要求

`drop-cli` 不应直接包含复杂业务逻辑。需要优先把以下能力拆入 `drop-sdk`：

- `config`: 配置读取、地址解析、密钥装载。
- `contracts`: Hub / Channel / verifier / OracleProxy client。
- `walrus`: 上传、下载、blob status 查询、blob id 转换。
- `oracle`: Worker health/status/blob-status/fulfill client。
- `proof`: VSS/VDD Prove Network provider。
- `workflow`: listing、purchase、fulfill、settle 阶段状态机。
- `state`: 本地状态文件读写和版本迁移。
- `tx`: 交易提交、receipt 刷新、replacement / revert / confirmation 判断。

CLI 只负责：

- 参数解析。
- 用户确认。
- 调用 SDK。
- 打印结果。
- 退出码。

## 测试验收标准

## 当前实施记录

2026-06-29 已完成第一版原型骨架：

- 新增 workspace crate：`drop-cli/`。
- `drop-cli` 固定原型目标为 Arbitrum Sepolia + 中心化 Oracle Worker。
- `drop-cli` 当前不使用 clap，先用轻量手写参数解析，减少新依赖。
- `drop-sdk` 新增基础模块：
  - `config`: 从 `drop-script/.env` 读取原型配置。
  - `oracle`: Oracle Worker `/health`、`/status`、`/walrus/blob-status` client。
  - `state`: sale 状态文件、交易记录、阶段记录模型。
- `drop-cli` 已实现并验证基础入口：
  - `drop-cli help`
  - `drop-cli init`
  - `drop-cli doctor`
  - `drop-cli status <sale-id>`
  - `drop-cli next <sale-id>`
  - `drop-cli oracle check <sale-id|--blob-id <id>|--c-cipher <0x...>>`
  - `drop-cli asset prepare <file>`
  - `drop-cli asset upload <sale-id>`
  - `drop-cli phase prepare <file>`
  - `drop-cli tx status <tx-hash>`
- `drop-cli` 已实现需要显式 `--yes` 的链上命令路径：
  - `drop-cli channel create [sale-id] --yes`: 调用 Hub `createExchangeChannel`，解析 `ExchangeChannelCreated`，可写回 sale state。
  - `drop-cli sale list <sale-id> --yes`: 读取 sale state，调用 Channel `listFile`，计算链上 sale id 和 `dataVersion`，写入交易记录。
- `drop-cli` 已建立安全门禁入口：
  - `drop-cli proof vss <sale-id>`: 当前提示将来会用 SP1 Prove Network。
  - `drop-cli proof vdd <sale-id>`: 当前提示将来会用 SP1 Prove Network。
  - `drop-cli settle <sale-id>`: 当前提示将来会发 Arbitrum Sepolia 交易。
  - `drop-cli recover-test <sale-id>`: 当前提示为开发验证命令。
  - `drop-cli phase publish <sale-id>`: 已串联 `asset upload`，之后停在 channel/listing 门禁。
  - `drop-cli phase prove <sale-id>`: 当前为门禁提示。
  - `drop-cli phase settle <sale-id>`: 当前为门禁提示。
  - `drop-cli phase verify <sale-id>`: 当前为门禁提示。
  - `drop-cli tx resume <sale-id>`: 当前为状态刷新入口提示。
- `drop-cli asset prepare <file>` 已补齐本地准备逻辑：
  - 按 `SYMBOL_SIZE` padding 原始文件。
  - 使用开发期默认 `asset_encryption_key = [0x22; 32]`，可由 `ASSET_ENCRYPTION_KEY` 覆盖。
  - 使用 `derive_rslh_nonce(asset_key, b"maenad_v1")`。
  - 使用 ChaCha8 加密 padded payload。
  - 计算 `original_asset_id` 和 `encrypted_blob_id`。
  - 写入本地密文文件和 sale state。
- `drop-cli asset upload <sale-id>` 已实装：
  - 读取 sale state 中的密文文件。
  - 使用 `WALRUS_PUBLISHER_URL` 或 `WALRUS_LOCAL_ENDPOINT` 上传。
  - 保存 `walrus_blob_id`。
  - 上传后尝试通过 Oracle Worker 查询 blob status。
  - 该命令会消耗 Walrus 存储，验证阶段未执行。
- 新增全功能串行测试脚本：
  - 脚本路径：`drop-cli/scripts/test-drop-cli-full-flow.sh`。
  - 用一个新生成的测试文件驱动 `drop-cli` 主流程，除非通过 `--asset FILE` 指定已有文件。
  - 读取 `drop-script/.env` 中的 `SELLER_KEY` 和 `BUYER_KEY`，只打印对应地址，不打印私钥。
  - 默认状态目录为 `/tmp/drop-cli-e2e-state`，每次运行的临时文件放在 `/tmp/drop-cli-e2e-*`。
  - 所有步骤串行执行，不并行构建、上传、发交易或请求证明。
  - 自动检查 sale state 中是否已有 `walrusBlobId`；已有则跳过上传，没有则只有在传入 `--yes-walrus` 时才上传。
  - VSS 和 VDD 证明阶段调用 `guest/scripts/zk-proof-test.sh <vss|vdd> prove`，只使用 SP1 Prove Network，不使用本地 proving。
  - 若 SP1 prove 失败，脚本会立即停止，不自动重试。
  - 资源消耗开关：`--yes-walrus`、`--yes-chain`、`--yes-prove`、`--yes-preflight`。

验证：

- `cargo fmt -p drop-sdk -p drop-cli` 通过。
- `cargo check -p drop-sdk` 通过。
- `cargo check -p drop-cli` 通过。
- `cargo build -p drop-cli` 通过。
- `target/debug/drop-cli help` 通过。
- 使用 `/tmp/drop-cli-test.env` 验证 `drop-cli init`，确认能读取 `DROP_CLI_ENV` 和 `DROP_CLI_STATE_DIR`。
- 使用 `/tmp/drop-cli-asset.txt` 验证 `drop-cli asset prepare`，生成 sale state、密文文件、`original_asset_id` 和 `encrypted_blob_id`。
- 使用临时 sale state 验证 `drop-cli status` 和 `drop-cli next`。
- 使用已知 active Walrus blob 验证 `drop-cli oracle check --blob-id ...`，返回 `status=0`、`expired=false`。
- `drop-cli doctor` 已只读访问 Arbitrum Sepolia RPC 和 Oracle Worker，返回 chain id 421614，Worker ok，relayer ready。
- 验证了 `drop-cli channel create` 和 `drop-cli proof vss <sale-id>` 的安全门禁输出。
- `drop-cli channel create [sale-id] --yes` 和 `drop-cli sale list <sale-id> --yes` 已在 Arbitrum Sepolia 实际发送并确认。

2026-06-29 已实跑 `drop-cli/scripts/test-drop-cli-full-flow.sh --asset /tmp/drop-cli-e2e-20260629-180905/drop-cli-e2e-asset.txt --yes-walrus --yes-chain --yes-prove --yes-preflight`：

- 第一次运行停在 Walrus 上传，原因是本地 publisher `http://localhost:31415` 未启动。
- 启动 `/home/justin/walrus/start.sh` 后重跑同一个 asset。
- `drop-cli doctor` 通过：Arbitrum Sepolia chain id 421614，Oracle Worker health/status 正常。
- `phase prepare` 通过：
  - local prepare sale id: `0x610b6b27821d18d2cd93fee37581401a693af072a1487c65dad7306f8a4b3dcf`
  - `originalAssetId`: `0xf68e6ba145c14bf13d03ea96c6b1f781eab0da58139de9ed003c49cc517a5a1e`
  - `encryptedBlobId`: `0xe7231a72940cbacac8c39a827b73f19989be715ef48b52bb8f0f05a7f177b2e8`
- Walrus 上传通过：
  - `walrusBlobId`: `5yMacpQMusrIw5qCe3PxmYm-cV70i1K7jw8Fp_F3sug`
  - Oracle Worker blob status: `active`，`status=0`，`expired=false`，`endEpoch=34`。
- Arbitrum Sepolia 链上交易通过：
  - `createExchangeChannel` tx: `0x01a069e5a1a4a601748631250dc86a383e55b877657dbfdc2d04e8c2834e5ea7`
  - channel: `0x88d82c8426c9e0a0ccc6e369400f072df3cd05d4`
  - `listFile` tx: `0xe0fe003f87bc6980706d8cbc98909517698dbe2f9fbc8b24c2e10c15c4d3db45`
  - on-chain sale id: `0x5a8d6c460b198ee4b4b7fa9ed000414c40adce75a0827c4896230eea982f13b2`
  - `dataVersion`: `0xceb2de6f262e5fb998107e280d37e19b3e4e520a913ffc5a09e8611338950f0f`
- VSS Prove Network Groth16 证明通过，fixture 已更新。
- VDD walrus_rslhve Prove Network Groth16 证明通过，fixture 已更新。
- VSS 和 VDD Arbitrum Sepolia official SP1 gateway preflight 均返回 `0x`。
- 发现并修正脚本问题：`sale list` 后合约事件返回的 on-chain sale id 会替换 local prepare sale id，脚本后半段必须切换到 on-chain sale id 后再执行 `status/proof/settle/tx resume`。
- 补充 settlement resume 脚本：
  - 脚本路径：`drop-cli/scripts/resume-drop-cli-sale-settlement.sh`。
  - 只从已有 sale state 继续，不重新 prepare、不重新上传 Walrus、不重新创建 channel、不重新 list。
  - 串行执行：`submitDataKeyCommitment -> buyer purchase -> seller fulfill(VSS+VDD) -> centralized oracle worker -> settle`。
  - `drop-cli/scripts/test-drop-cli-full-flow.sh` 新增 `--yes-settle` 开关，可在 full-flow 中显式进入 settlement 阶段。
- 补充 `drop-script` resume binary：
  - 路径：`drop-script/src/bin/resume_drop_cli_sale.rs`。
  - 读取 drop-cli state，构造当前 sale 的 `ListingState`，复用 `drop-script` 阶段函数执行剩余链路。
  - `drop-script` 新增 `DROP_SCRIPT_INPUT_ASSET` 覆盖项，避免 VDD proof 继续读取硬编码 Apollo 视频，确保 proof 输入来自当前 sale state 的实际资产文件。
- 2026-06-29 执行 settlement resume：
  - 已发送 `submitDataKeyCommitment` tx: `0xcc2b81c714320d2f6b50c13684517df69c220099771033bdc1fe6ded15954eaa`
  - 已发送 buyer `purchase` tx: `0xc249bb439c7f28142169723b765b88aa8296afc94e378673ee5673bc5e3c4fd0`
  - 未发送 fulfill/oracle/settle。
  - 停止原因：seller fulfill 前证明阶段失败。VSS Prove Network 返回 `Proof request 0x4e24a6022a2bb5c272dabd3502abc912a46acb07a28bd766ace816eeb364262d is unexecutable`；同次运行中 VDD proof 生成成功，但合约 verifier 模拟返回 `InvalidPublicValues()`。
  - 已修正 `stage_3_fulfill`：VSS proof 或 VSS verifier 模拟失败后立即返回，不再继续请求 VDD proof。
  - 发现状态设计问题：buyer purchase 后 `ephemeral_pubkey` 没有写入 drop-cli state；如果进程在 purchase 后失败，不能安全恢复同一笔 purchase 的 seller fulfill。后续必须把 purchase context 持久化，或者把 purchase/fulfill 设计成单次不中断的原子阶段。

未执行：

- 未运行 `asset prepare` 处理大视频，避免触发耗时 Walrus blob id 计算。
- 未通过 `drop-cli proof vss/vdd` 自身提交证明；当前 full-flow 脚本用 `guest/scripts/zk-proof-test.sh` 跑 VSS/VDD Prove Network 和 preflight。
- settlement resume 已执行到 buyer purchase，但未完成 fulfill、Oracle Worker fulfill、settle 和 recover-test 的真实闭环。

当前剩余实施：

- `proof vss/vdd` 需要从门禁提示升级为调用现有 SP1 Prove Network proof flow，并严格防止失败后盲目重试。
- `phase prove/settle/verify` 需要在 proof、Oracle fulfill、settle 实装后串联。
- `tx resume` 需要读取状态中的交易记录并刷新 receipt，而不是仅提示 `drop-cli status`。
- 需要补 purchase context 持久化，至少保存 fulfill 所需的 buyer purchase tx、secret sharing key 派生参数和 ephemeral pubkey。
- 需要排查 VSS Prove Network `unexecutable` 和 VDD verifier `InvalidPublicValues()` 的根因，再由用户批准后重新请求证明。
- 需要补 Oracle Worker fulfill、settle、recover-test 的真实命令实现。

2026-06-30 已补齐原型 full-flow 缺口并完成一次真实全流程测试：

- 明确 `drop-cli` 不承载 buyer 前端 `purchase` 原子命令；当前 full-flow 原型中 buyer purchase 只作为测试流程的一部分，由 `complete-test-flow` 桥接脚本执行。
- 补充显式 bridge binary：`drop-script/src/bin/resume_drop_cli_sale.rs`。
  - 该 binary 是 0009 原型过渡层，不是隐藏入口。
  - 它读取 `drop-cli` sale state，复用 `drop-script` 已跑通的阶段函数完成 `submitVDDProof -> oracle worker -> buyer purchase -> fulfill -> oracle worker -> settle -> recover`。
  - 后续产品化时，应把这部分流程拆回 `drop-sdk` workflow，再由 `drop-cli` 原生命令调用。
- `drop-script` 阶段函数调整：
  - `stage_1_6_submit_vdd_proof` 可被 bridge binary 调用。
  - `stage_5_settle` 返回 settle tx hash，便于写入 drop-cli state。
- `drop-cli` state 补充 `original_len`，用于恢复时截断 padding 后的明文。
- `asset upload` 默认 Walrus 保存 epoch 从 `1` 调整为 `4`，可通过 `DROP_CLI_WALRUS_EPOCHS` 覆盖。
  - 原因：一次测试中 1 epoch 上传后 Worker 返回 `expired`，继续证明会浪费 Prove Network 请求。
  - 现在上传后会强制查询 Oracle Worker blob status，只有 `found=true`、`status=0`、`expired=false` 才允许继续。
- `drop-cli status/next` 修正 settle 后的下一步判断：
  - `settle` 已 confirmed 时，下一步为 `drop-cli phase verify <sale-id>`。
  - 不再错误提示继续 `phase prove`。
- `drop-cli/scripts/test-drop-cli-full-flow.sh` 调整：
  - 默认每次使用独立 `DROP_CLI_STATE_DIR=$RUN_DIR/state`，避免旧 state 污染。
  - 串行执行，不并行上传、证明或链上交易。
  - 结尾增加恢复文件 hash 对比，确保 recovered asset 与原始 asset 内容一致。

2026-06-30 真实测试命令：

```bash
drop-cli/scripts/test-drop-cli-full-flow.sh --yes-walrus --yes-chain --yes-prove --yes-oracle --yes-settle
```

测试环境：

- Arbitrum Sepolia。
- Walrus mainnet publisher：本地 `/home/justin/walrus/start.sh` 提供的 publisher endpoint。
- Oracle：中心化 Oracle Worker。
- SP1：Prove Network，未使用本地 proving。
- run dir：`/tmp/drop-cli-e2e-20260630-151101`。
- state dir：`/tmp/drop-cli-e2e-20260630-151101/state`。

本次 full-flow sale：

- sale id：`0xf233787bf2ce13674def62efa65a81609033bad584bcf861d02d800208564a01`
- channel：`0x60af842e015ca84573752e51f815710462f6eb81`
- Walrus blob id：`eOoGKz_sTuLcLt_QAycdIWll1DKFES2GEodVxphyY80`
- Oracle Worker blob status：`status=0`，`expired=false`，`endEpoch=37`

链上交易：

- `channel_create`：`0xe5d4fa7f1d1da590ffad9b260da549032321974d0f3c091c29ca7f7ed6e42112`
- `sale_list`：`0x2612d9c06946edcf4a49d5c78297048d1e434a1628df52031422c2556224d3d2`
- `submit_data_key_commitment`：`0x846a98401eacb7c2acbb19b3f53878b8cd3a1f412a196e8f6f6b953f20ee8993`
- `submit_vdd_proof`：`0x3e1d04bd311228fe8fa95d9da2345b6ee749a033479c917e3d6c8cd408f841ea`
- Oracle Worker report tx：`0x953bbf1cfa13cf5bb2364fdbc3546a877c03bfc13b148c3f956d776728a3a5fd`
- buyer `purchase`：`0xed425ab2f7c883b9a3c3732d1781768f4a102a3419d7ed08ce5e219b40f37150`
- seller `fulfill`：`0xf7e4cbb4deff8c8db8f6e848b3b9f728e316667e703f31e7fa24e8db3e9db095`
- seller `settle`：`0x717fc53fff14bfbac2b587b30b88910b7ce60b29f3161666ea8258f29fc180a8`

证明和验证：

- VDD Prove Network Groth16 证明生成成功。
- VDD wrapper simulation 成功。
- VSS Prove Network Groth16 证明生成成功。
- VSS wrapper simulation 成功。
- `drop-cli status <sale-id>` 显示所有关键交易均为 `Confirmed`。
- settle 后 next action 正确为 `drop-cli phase verify <sale-id>`。

恢复验证：

- 恢复文件：`KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile-recovered.mp4`
- 原始文件 hash：`32a991b1aaef858dab6759d5b131533ec6c782eb8e81dc0306d7f0cbd8daf334`
- 恢复文件 hash：`32a991b1aaef858dab6759d5b131533ec6c782eb8e81dc0306d7f0cbd8daf334`
- 结论：恢复内容与原始内容一致。

本次没有重跑完整流程的原因：

- 已经完成一次真实 full-flow，且包含 Walrus 上传、SP1 Prove Network、Arbitrum Sepolia 交易、Oracle Worker fulfill、settle、recover hash 对比。
- 后续只做了 `drop-cli status` 和本地 hash 对比，不再次消耗 Prove Network / Walrus / 链上资源。

### 单元测试

- 配置解析。
- 状态文件读写。
- Walrus blob id / cCipher 编码转换。
- Oracle Worker response 解析。
- 合约事件解析。

### 本地集成测试

- 使用 mock provider 或本地 Anvil 测合约调用编码。
- 使用 mock HTTP server 测 Oracle Worker client。
- 使用临时目录测试状态恢复。
- pending / confirmed / reverted / replaced 交易状态刷新。

### 测试链 preflight

- `drop-cli doctor` 对 Arbitrum Sepolia 做只读检查。
- Oracle Worker `/status`。
- Walrus `/walrus/blob-status` active / expired / not found 三类查询。
- `drop-cli status` 能从链上刷新已有 tx 状态。
- `drop-cli next` 能根据状态给出正确下一步命令。

### Live flow

只在用户明确批准后执行：

- Walrus 上传。
- SP1 Prove Network 请求。
- 链上交易。
- Oracle Worker fulfill。

## 实施优先级

第一阶段：

- `drop-cli doctor`
- `drop-cli oracle check`
- `drop-cli asset prepare`
- `drop-cli asset upload`
- `drop-cli status`
- `drop-cli next`
- `drop-cli phase prepare`

第二阶段：

- `drop-cli channel create`
- `drop-cli sale list`
- `drop-cli oracle fulfill`
- `drop-cli tx status`
- `drop-cli tx resume`
- `drop-cli phase publish`

第三阶段：

- `drop-cli proof vss`
- `drop-cli proof vdd`
- `drop-cli settle`
- `drop-cli phase prove`
- `drop-cli phase settle`

第四阶段：

- `drop-cli run full-flow`
- `drop-cli recover-test`
- `drop-cli phase verify`

## 未决问题

- 正式环境是否继续支持中心化 Oracle Worker，还是只作为 prototype/devnet 选项。
- seller 是否需要内置 key management，还是只读取外部 wallet/private key。
- subgraph 在 `drop-cli` 中是强依赖还是可选加速查询。
- Walrus publisher 节点是否需要单独 skill 文档继续维护。

## 经验总结

待实施后补充。
