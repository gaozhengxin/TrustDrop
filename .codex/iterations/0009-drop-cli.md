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

### 下一阶段架构设计: TUI、daemon、数据库和交易 thread

2026-06-30 新增设计要求：

- 先写清楚架构，等待用户确认后再实施。
- 不重跑 full-flow。
- 不破坏 `drop-cli/scripts/test-drop-cli-full-flow.sh` 当前已经跑通的完整功能。
- 当前 bridge binary 已删除；`complete-test-flow` 直接调用 drop-script library。后续仍应继续把 workflow 下沉到 drop-sdk。

#### 已删除 bridge binary 的逻辑归属

早期 bridge binary 曾做了这些事情：

- 从 drop-cli state 读取 sale 上下文。
- 复用 `drop-script` 阶段函数。
- 执行 `submitVDDProof -> oracle worker -> buyer purchase -> fulfill -> oracle worker -> settle -> recover`。
- 把关键 tx 写回 drop-cli state。

它不应该长期留在 `drop-script/src/bin/` 中。正确归属应拆成三层：

- `drop-sdk::workflow`: 承载协议 workflow 状态机和业务步骤。
  - `submit_vdd_proof_for_sale`
  - `fulfill_purchase`
  - `trigger_oracle_for_request`
  - `wait_oracle_signal`
  - `settle_purchase`
  - `recover_for_test`
- `drop-sdk::store`: 承载持久化读取和写入。
  - 保存 channel、sale、purchase、proof、oracle request、tx、thread。
  - 提供 migration 和 schema version。
- `drop-cli`: 只做交互层。
  - 原子命令调用 `drop-sdk::workflow` 单步。
  - 阶段命令调用多个 workflow step。
  - TUI 和 daemon 也只调用 SDK，不直接复用 `drop-script`。

当前状态：

- bridge binary 已删除。
- `drop-cli phase complete-test-flow` 直接调用 `drop-script` library。
- `drop-cli proof vdd`、`drop-cli proof vss`、`drop-cli settle`、`drop-cli recover-test` 已从 placeholder 改为实际调用已验证阶段函数。
- 后续仍应继续把这些 workflow 下沉到 `drop-sdk`，让 `drop-script` 只保留脚本层入口。

#### 总体架构

目标形态：

```text
drop-cli
  commands      命令行入口，适合脚本和调试
  tui           seller 交互工作台
  daemon        后台自动跟踪和自动响应
  views         status / warning / next action 展示

drop-sdk
  config        配置读取、校验、profile
  store         SQLite 轻量数据库、migration、repository
  contracts     Hub / Channel / OracleProxy / verifier client
  chain         receipt、event scan、confirmation、nonce/tx 状态
  walrus        upload/download/blob status
  oracle        worker client、oracle request/report 状态
  proof         VSS/VDD Prove Network provider
  workflow      channel/sale/purchase/thread 状态机
  policy        daemon 自动响应规则
```

`drop-script` 的长期定位：

- 保留为开发验证脚本。
- 不再承担产品流程状态管理。
- 不再保存 fixture 作为主状态。
- 可以继续用于快速复现实验，但不能成为 `drop-cli` 的核心依赖。

#### 轻量数据库

需要用内置轻量数据库替代当前 JSON fixture/state 文件。建议默认使用 SQLite：

- 本地单文件，适合 CLI、TUI、daemon 共享。
- 支持事务，便于防止同一 purchase 被重复 fulfill 或 settle。
- 支持索引，便于 TUI 按 channel/sale/purchase/thread 查询。
- Rust 生态成熟，后续可用 `sqlx` 或 `rusqlite`。

建议默认路径：

```text
~/.trustdrop/drop.sqlite
```

项目开发环境可通过配置覆盖：

```text
DROP_CLI_DB=/tmp/drop-cli-e2e/drop.sqlite
```

数据库至少包含：

- `profiles`: 当前 seller profile、chain id、配置来源。
- `channels`: seller 拥有的 channel、创建 tx、当前状态。
- `sales`: sale/listing、asset id、data version、Walrus blob、价格、状态。
- `purchases`: buyer purchase request、purchase tx、buyer、sale id、purchase context、所属处理 thread、状态。
- `proofs`: VSS/VDD proof request、VK、public values hash、proof tx、状态。
- `oracle_requests`: request tx、request id、report tx、success until、状态。
- `transactions`: 所有链上 tx 的生命周期。
- `threads`: seller 对一组 purchase 的可恢复处理流程。
- `thread_purchases`: thread 和 purchase 的多对多/一对多成员关系。
- `warnings`: daemon 检测到的异常和待处理项。
- `events_cursor`: 每个 channel / Hub / subgraph 的扫描游标。

敏感数据规则：

- 不保存 seller private key。
- 不保存 SP1 private key。
- 不保存 Oracle Worker token。
- 不保存 Walrus API key。
- 明文 data key 默认不入库；如果开发期必须保存，需要显式 `dev_store_plaintext_keys=true`，并在 TUI/doctor 中醒目标记。

#### Purchase 和 Thread 模型

核心关系：

```text
channel
  sale/listing A
    purchase 1
    purchase 2
  sale/listing B
    purchase 3
      ...
  thread A: 处理一组需要 VSS 的 purchase batch
  thread B: 处理一个不需要 VSS 的单笔 purchase
```

`purchase` 是链上购买请求，是协议业务对象。它来自 buyer 的 purchase 交易，表示某个 buyer 对某个 sale 的购买意图和支付状态。

`thread` 是 CLI/daemon 在 seller 选择响应 purchase 时自动维护的处理流程。它不是 seller 手动创建的协议对象，也不是用户需要显式管理的工作单。用户在 TUI 中操作的是 purchase 或 purchase batch，例如“响应这些购买请求”；CLI 根据这些操作自动生成或复用一个 thread 来跟踪 VSS 批处理、fulfill、oracle 和 settle。

合约层面的 VSS 复用依据：

- `ExchangeChannel.fulfill` 只有在 `!isPrivy(buyer)` 时才调用 `shareDataKey` 并验证 VSS proof。
- `ExchangeChannel.settle` 只要求最终 `isPrivy(buyer)`、VDD verified、oracle success window 有效。
- `isPrivy` 来自 channel 内的 audience bitmap，不绑定单个 sale。因此同一 channel 下如果多个 asset 使用同一个 data key / `dataKeyCommitment`，buyer 成为 privy 后，后续 purchase 可以跳过 VSS。
- 合约源码已新增 `needsVSS(address)`、`audienceCount()`、`getAudienceVssKeyCommitments(address[])` 三个只读辅助函数，用于 CLI/daemon 判断 batch 和构造 VSS proof 输入。
- `drop-cli` 必须从 purchase tx receipt 解析 `PurchaseEvent`，拿到 `buyer` 和 `channel`，再调用 `needsVSS(buyer)` 判断 `needsVss`；如果链上仍是旧合约没有 `needsVSS`，CLI 应 fallback 到 `!isPrivy(buyer)`。
- 当 `needsVss=false`，CLI/daemon 应该为该 purchase 单独维护 thread，直接进入 fulfill/oracle/settle 路径。
- 当 `needsVss=true`，CLI/daemon 应按配置等待多个 purchase 进入 batch，再生成一次 VSS proof。未来完整实现应优先调用 channel 的 batch `shareDataKey`，再对每笔 purchase 执行 fulfill，使 fulfill 跳过 VSS。

为什么需要 thread：

- 一个 channel 下会有多个 purchase。
- 一次 VSS 可以处理多个 purchase 请求，不能把操作模型设计成每个 purchase 都孤立处理。
- 不是所有 purchase 都需要 VSS；已 privy 的 buyer 要走单笔 no-vss thread，避免等待无意义的 batch。
- fulfill 之后会触发 oracle request，需要跟踪 request/report/success window。
- settle 依赖 VDD、VSS/fulfill、oracle signal、purchase 状态，必须把这些条件合并展示。
- TUI 需要给 seller 一个易于理解的响应流程视图，而不是要求 seller 理解或手动创建 thread。

purchase 状态：

- `detected`: 已从链上或 subgraph 发现。
- `paid`: purchase tx 已确认，等待 seller 响应。
- `queued`: 已加入某个 thread，等待批处理。
- `vss_ready`: 所属 thread 的 VSS proof 已生成并通过校验。
- `fulfill_submitted`: fulfill tx 已提交。
- `fulfilled`: fulfill tx 已确认，`DataKeyShared` 已发出。
- `oracle_pending`: fulfill/listing 相关 oracle request 已触发，等待 report。
- `settle_ready`: oracle success window 有效，且合约状态满足 settle。
- `settle_submitted`: settle tx 已提交。
- `settled`: settle 已确认。
- `blocked`: 需要人工处理。
- `failed`: 当前处理失败，等待诊断。

thread 类型：

- `PublishSaleThread`: prepare / upload / channel create / list / VDD proof。
- `PurchaseFulfillmentThread`: 对同一 channel/sale 下的一组 purchase 做 VSS batch、fulfill、oracle、settle。
- `RecoveryTestThread`: 开发期恢复验证。

`PurchaseFulfillmentThread` 状态：

- `planned`: seller 已选择响应一组 purchase，CLI 已自动建立 thread。
- `ready`: 已有 purchase 可处理，等待 CLI/daemon 执行或等待用户确认资源消耗动作。
- `proving_vss`: 正在为该组 purchase 请求 VSS proof。
- `vss_ready`: VSS proof 已完成，可以提交 fulfill。
- `fulfilling`: 正在提交 fulfill tx。
- `fulfilled`: fulfill tx 已确认。
- `oracle_pending`: 等待 Oracle Worker report 或链上 oracle signal。
- `settle_ready`: 一个或多个 purchase 已可 settle。
- `settling`: 正在提交 settle tx。
- `completed`: thread 内所有 purchase 已 settled 或被明确跳过。
- `blocked`: 缺少余额、证明失败、oracle 超时、nonce/pending tx 异常等，需要人工处理。
- `failed`: 自动流程失败，必须人工诊断后才能 resume。

每个 thread 至少记录：

- `thread_id`
- `kind`
- `channel_address`
- `sale_id`
- `purchase_count`
- `purchase_ids`
- `batch_policy`: seller 选择的 purchase batch、daemon 规则命中的 batch、按时间窗口或数量阈值形成的 batch。
- `current_step`
- `status`: 使用 `PurchaseFulfillmentThread` 状态枚举，例如 `planned` / `ready` / `proving_vss` / `vss_ready` / `fulfilling` / `oracle_pending` / `settle_ready` / `settling` / `completed` / `blocked` / `failed`
- `vss_proof_id` 可选
- `fulfill_tx_hash` 可选
- `oracle_request_ids`
- `settle_tx_hashes`
- `lock_owner` 和 `lock_until`
- `last_error`
- `next_action`
- `created_at`、`updated_at`

thread 和 purchase 的状态同步规则：

- seller 选择响应 purchase 后，CLI 自动创建或复用 thread，purchase 进入 `queued`。
- thread 的 VSS proof 成功后，所有成员 purchase 可进入 `vss_ready`。
- fulfill tx 确认后，所有被该 fulfill 覆盖的 purchase 进入 `fulfilled` 或 `oracle_pending`。
- oracle report 成功后，满足条件的 purchase 进入 `settle_ready`。
- settle 可以逐个 purchase 执行，也可以由 thread 批量驱动多笔 settle。
- 如果 thread 中部分 purchase 失败，不能阻塞其它已满足条件的 purchase settle；TUI 必须显示 partial success。

#### 2026-06-30 实施进度: VSS 复用与密钥检查

已完成：

- `contracts/src/VSS.sol` 新增 VSS 复用辅助 view：`needsVSS`、`audienceCount`、`getAudienceVssKeyCommitments`。
- `contracts/test/BitmapTest.t.sol` 增加 batch helper 测试，覆盖新 audience 需要 VSS、批量 `shareDataKey` 后跳过 VSS、批量读取 commitment。
- `drop-sdk` hardcoded ABI 已同步新增 view。
- `drop-cli purchase show` / `phase respond` 已从 purchase receipt 解析 `PurchaseEvent`，读取 buyer/channel，并按 `needsVSS` 或旧合约 fallback `!isPrivy` 写入 thread。
- `drop-cli keys check` 已新增，只输出 seller address、owner public key、asset key commitment，不输出私钥。
- `maenad_v1` nonce domain 已替换为 `trustdrop_asset_v1`，并同步 `drop-cli`、`drop-script` 和 VDD walrus_rslhve host scripts。

仍未完成（历史记录，2026-07-01 batch VSS 更新后下列前两项已完成）：

- 已完成更新：原生 `drop-cli phase fulfill <thread-id>` 和 `phase settle <thread-id>` 已替代单 purchase bridge 路径，支持 thread 内多 purchase。
- 已完成更新：原生 batch VSS proof 已能在 CLI thread fulfill 中生成并提交 `shareDataKey`。
- buyer purchase 的跨进程元数据仍缺失。现有 `drop-script` bridge 依赖内存中的 `purchase.secret_sharing_key` 和 `purchase.ephemeral_pubkey`；如果 purchase 来自独立 buyer 前端，CLI 必须从链上事件或链下订单元数据拿到解封装所需材料。
- SQLite/TUI/daemon 尚未实现。
- 密钥管理仍未达到完整产品级 keystore。当前已经禁止静默默认 key，但还没有加密 keystore、外部 signer、硬件钱包、密钥轮换和 profile 权限模型。

当前配置缺口：

- `drop-script/.env` 当前缺少 `OWNER_SECRET_KEY` 和/或 `ASSET_ENCRYPTION_KEY` 时，`drop-cli keys check` 会失败；测试脚本可显式设置 `TRUSTDROP_DEV_INSECURE_DEFAULT_KEYS=1`，但 seller 正式操作不能使用该开关。

合约部署影响：

- 本轮合约源码已更新，但 Arbitrum Sepolia 上的当前 Hub 仍指向旧 `Exchange logic` 地址。
- 因为 `ExchangeHub.implementation()` 是 immutable，若要让新建 channel 也具备新增 view，需要重新部署 `ExchangeChannelImplementation` 和 `ExchangeHub`，然后更新 `drop-script/.env`、`contracts/deployed.md`、subgraph manifest start block/address。
- 旧 channel 没有新增 view，但 `drop-cli` 已保留 fallback，因此不强制立即重新部署才能继续测试现有链上 flow。

resume 规则：

- 每一步必须幂等。执行前先读 DB 和链上状态。
- 对已经 confirmed 的 tx 不重复发送。
- 对已经 submitted 但 pending 的 tx 只刷新 receipt，不用新 nonce 覆盖。
- 对证明请求失败，默认停止并要求人工确认。
- 对 Oracle Worker 失败，记录 warning，不盲目循环触发。
- 对 nonce/replacement 不确定状态，标记 `blocked`，等待人工检查。
- resume 的主要用户入口是 purchase 或 purchase batch；实现上 CLI 找到对应 thread，没有则从 purchase 状态自动恢复出 thread。

#### Channel 状态管理

seller 需要管理所有自己的 channel，而不是只管理当前一次测试 sale。

核心状态：

- channel 是否已创建。
- channel 所属 seller 是否匹配当前 profile。
- channel 当前 open sale 数量。
- 每个 sale 是否已 list、VDD verified、Oracle verified。
- 每个 purchase 是否需要 seller 响应。
- 每个 purchase 是否已经 fulfill。
- 每个 purchase 是否可以 settle。
- 每个 purchase 是否已经 settle 或异常。

状态来源按优先级组合：

- 本地 DB：保存执行历史、purchase 状态和 thread 状态。
- 链上 RPC：权威确认 tx、events、contract view。
- subgraph：加速查询和列表视图，但不能作为唯一可信来源。
- Oracle Worker：补充 Walrus blob status、relayer/pending 状态。
- Walrus：补充 blob active/expired。

#### TUI 设计

TUI 是 seller 的交互工作台，目标是让 seller 看到 channel 上的购买请求，并选择对哪些 purchase 做响应。thread 是 CLI 背后的流程跟踪对象，由 CLI 自动维护；TUI 展示 thread 进度，但不要求 seller 手动创建 thread。

入口：

```text
drop-cli tui
```

主要视图：

- Dashboard：profile、chain、余额、daemon 状态、warning 数量。
- Channels：所有 seller channel、同步状态、open sale/purchase 数。
- Sales：每个 sale 的 Walrus、VDD、oracle、purchase 概览。
- Purchases：购买请求列表，重点显示未入队、已入队、待 fulfill、待 settle。
- Threads：seller 响应流程列表，显示每个自动维护 thread 覆盖的 purchase 数量、VSS、fulfill、oracle、settle 进度。
- Thread Detail：某个 thread 的 purchase 成员、批处理 proof、fulfill tx、oracle request、settle tx。
- Warnings：daemon 发现的异常、建议命令、是否需要人工处理。
- Logs：最近 daemon/CLI 操作日志，不显示 secret。

TUI 中允许的操作：

- refresh：只读刷新 channel/sale/purchase 状态。
- respond selected：选择一个或多个同一 channel/sale 下的 paid purchase，CLI 自动创建或复用 thread 并进入响应流程。
- add selected to thread：把新的 purchase 加入 CLI 判定仍可合并的 thread。
- prove VSS：为 thread 中的 purchase batch 生成一次 VSS proof。
- fulfill thread：对 thread 覆盖的 purchase 提交 fulfill。
- retry oracle：对 thread 中明确失败且允许重试的 oracle request 触发一次。
- settle ready：对 thread 中满足条件的 purchase 执行 settle。
- open detail：查看 tx、proof、oracle、Walrus 状态。
- mark reviewed：把 warning 标记为已读。

TUI 展示原则：

- 默认按 channel 分组，再按 sale 分组，再显示 purchases 和 threads。
- purchase 列表强调“是否需要 seller 操作”。
- thread 列表强调“下一步是什么”和“这批 purchase 当前处理到哪里”。
- 对 partial success 要直接展示：例如 10 个 purchase 中 8 个 settled、1 个 oracle pending、1 个 blocked。
- 用户不应该被迫理解每个底层表；TUI 文案应该围绕“购买请求”“处理批次”“等待证明”“等待预言机”“可以结算”。

安全规则：

- 会发链上交易、请求 Prove Network、触发 Oracle Worker 的操作必须弹确认。
- 默认不自动重试证明。
- TUI 不能显示私钥、token、明文 data key。
- TUI 操作也必须通过同一套 `drop-sdk::workflow` 和 DB lock，不能绕过 daemon/CLI 的幂等规则。

#### Daemon 设计

daemon 用来自动跟踪 seller 所有 channel 状态，并按配置规则响应。

入口：

```text
drop-cli daemon run
drop-cli daemon status
drop-cli daemon stop
```

daemon 是单机本地进程，不要求高可用。它必须用 DB lock 防止和 TUI/CLI 同时处理同一 thread。

配置示例：

```toml
[daemon]
enabled = true
poll_interval_secs = 30
warning_interval_secs = 300
max_concurrent_threads = 1

[daemon.channels]
auto_discover = true
include = []
exclude = []

[daemon.policy]
auto_respond = false
auto_fulfill = false
auto_settle = false
auto_trigger_oracle = true
require_manual_approval_for_proof = true
require_manual_approval_for_settle = true

[daemon.batch]
max_purchases_per_thread = 16
max_thread_wait_secs = 120
only_batch_same_sale = true

[daemon.limits]
min_seller_eth_wei = "10000000000000000"
min_oracle_relayer_eth_ok = true
max_pending_tx_age_secs = 600
max_oracle_wait_secs = 900

[daemon.scan]
use_subgraph = true
fallback_rpc_logs = true
purchase_refresh_secs = 20
sale_refresh_secs = 60
walrus_refresh_secs = 120
oracle_refresh_secs = 30
```

刷新职责：

- 定期发现 seller channel。
- 定期扫描 channel 上的新 purchase。
- 按 batch 规则判断哪些 purchase 可以一起响应；默认只提示 seller，不自动响应。
- 刷新 pending tx receipt。
- 刷新 VDD/VSS proof 状态。
- 刷新 Oracle request/report 状态。
- 刷新 Walrus blob active/expired。
- 生成 warning。

自动响应规则：

- `auto_respond=false` 时，只发现 purchase 和生成 warning，由 seller 在 TUI/CLI 中选择是否响应。
- `auto_respond=true` 时，daemon 可按同一 channel/sale、时间窗口、数量阈值自动选择 purchase 并创建 thread。
- `auto_fulfill=false` 时，只维护待处理 thread 和 warning，由 TUI/CLI 人工执行。
- `auto_fulfill=true` 时，可自动执行 thread 的 fulfill，但如果需要新的 SP1 proof 且 `require_manual_approval_for_proof=true`，必须阻塞等待确认。
- `auto_settle=false` 时，只提示可 settle。
- `auto_settle=true` 时，可对 thread 中满足 `vddVerified && isPrivy && oracleSuccessUntil valid` 的 purchase 自动 settle。
- Oracle Worker trigger 可以默认自动，因为它不使用 seller 私钥，但仍需记录 report tx 和 warning。

warning 类型：

- seller ETH 余额低。
- Oracle Worker relayer 余额不足或有 pending tx。
- Walrus blob 已过期或即将过期。
- purchase 等待 fulfill 超时。
- oracle request 等待 report 超时。
- settle 条件满足但长时间未执行。
- tx pending 时间过长。
- proof request failed。
- DB lock 过期或 thread 状态不一致。
- subgraph lagging，已切回 RPC logs。

#### CLI 三层操作面

CLI 必须同时提供三套操作方式。三套方式调用同一套 SDK workflow 和数据库状态机，不能各自维护一套逻辑。

第一层是 primitive 命令：每个命令只做一次底层动作，便于调试、定位和恢复。

```text
drop-cli db init
drop-cli db migrate
drop-cli db inspect

drop-cli channel list
drop-cli channel show <channel>
drop-cli channel sync
drop-cli channel watch <channel>
drop-cli channel create

drop-cli sale list [--channel <channel>]
drop-cli sale show <sale-id>

drop-cli purchase list [--channel <addr>] [--sale <id>]
drop-cli purchase show <purchase-tx>
drop-cli purchase settle <purchase-tx>

drop-cli asset prepare <file>
drop-cli asset upload <sale-id>
drop-cli sale list <sale-id>
drop-cli proof vdd <sale-id>
drop-cli proof vss <thread-id>
drop-cli fulfill <thread-id>
drop-cli oracle fulfill <tx-hash>
drop-cli settle <purchase-tx>

drop-cli thread list [--channel <addr>] [--sale <id>]
drop-cli thread show <thread-id>
drop-cli thread cancel <thread-id>
```

primitive 命令规则：

- 可以发单笔链上交易、上传一次 Walrus、请求一次证明、触发一次 Oracle Worker。
- 每次资源消耗动作都必须有确认或显式 `--yes`。
- 不自动选择 purchase batch。
- 不自动推进下一阶段。
- 写入 DB，供 phase、TUI、daemon 继续识别。

第二层是 phase 复合命令：把一个阶段里 seller 通常需要连续做的 primitive 串起来。

```text
drop-cli phase prepare <file>
drop-cli phase publish <sale-id>
drop-cli phase prove-sale <sale-id>
drop-cli phase respond <purchase-tx>...
drop-cli phase fulfill <thread-id>
drop-cli phase settle <thread-id>
drop-cli phase verify <sale-id|thread-id>
```

phase 命令规则：

- `phase prepare`: 本地准备，不上传、不发交易、不证明。
- `phase publish`: upload Walrus、create channel if needed、list sale、submit VDD proof。
- `phase respond <purchase-tx>...`: seller 选择响应一批 purchase，CLI 自动创建或复用 thread，但不要求 seller 手动创建 thread。
- `phase fulfill <thread-id>`: 为 thread 生成 VSS proof，提交 fulfill，触发必要 oracle request。
- `phase settle <thread-id>`: 刷新 oracle signal，对满足条件的 purchase 执行 settle。
- 每个 phase 必须能从 DB 中识别已完成步骤，不重复发 confirmed tx。
- Prove Network 失败后停止，不自动重试。

对象发现和 id 获取规则：

- `purchase-tx` 来自 `drop-cli purchase list` 或 `drop-cli purchase show`。
- `thread-id` 来自 `drop-cli phase respond <purchase-tx>...` 的输出，或来自 `drop-cli thread list`。
- seller 不应该被要求凭记忆输入 id；每个 list/show/phase 命令都必须打印下一步建议。
- `phase respond` 必须输出它创建或复用的 thread：

```text
thread: th_20260630_000001
channel: 0x...
sale: 0x...
purchases: 2
next: drop-cli phase fulfill th_20260630_000001
```

推荐手动运营路径：

```text
drop-cli channel list
drop-cli sale list --channel <channel>
drop-cli purchase list --channel <channel> --sale <sale-id> --status paid
drop-cli phase respond <purchase-tx-1> <purchase-tx-2>
drop-cli thread show <thread-id>
drop-cli phase fulfill <thread-id>
drop-cli thread show <thread-id>
drop-cli phase settle <thread-id>
drop-cli thread show <thread-id>
```

`thread show <thread-id>` 必须展示：

- thread id、channel、sale、status、next action。
- thread 内 purchase 列表：tx hash、buyer、amount、purchase 状态、settle 状态。
- VSS proof 状态：not requested / proving / ready / failed。
- fulfill 状态：tx hash、receipt、是否发出 `DataKeyShared`。
- oracle 状态：request tx、report tx、success window、失败原因。
- settle 状态：每个 purchase 的 settle tx、confirmed/reverted/pending。
- partial success：例如 10 个 purchase 中 8 个 settled、1 个 oracle pending、1 个 blocked。

第三层是 daemon：定期自动发现 purchase，并按规则自动划分 batch、自动维护 thread。

```text
drop-cli daemon run
drop-cli daemon status
drop-cli daemon check
drop-cli tui
```

daemon 操作规则：

- 定期同步 channel/sale/purchase 状态。
- 按配置自动把购买请求划分 batch。
- batch 命中后自动创建或更新 thread。
- 是否自动证明、fulfill、settle 由配置决定。
- 默认不自动发 seller 交易、不自动请求证明，只生成 thread 建议和 warning。

TUI 是这三层之上的交互界面：

- 可以直接调用 primitive 动作。
- 可以触发 phase 复合动作。
- 可以查看 daemon 自动维护的 thread 和 warning。

不把 `thread resume` 作为主命令。原因：

- thread 不是用户手动创建的对象，用户不应该需要“恢复一个抽象 thread”才能继续。
- 正常恢复入口应该是 `phase fulfill <thread-id>`、`phase settle <thread-id>` 或 TUI 中的 continue action。
- 如果实现层需要 `thread resume`，只能作为 debug/maintainer 命令，含义是：读取 DB 中一个 `blocked/failed` thread，刷新链上 receipt、proof、oracle 状态，计算下一步，不默认发交易、不请求证明。

可选 debug 命令：

```text
drop-cli debug thread resume <thread-id>
```

`debug thread resume` 不进入普通 seller CLI 主流程。

#### 当前已有命令的处理

- `status <sale-id>` 保留，但数据源逐步从 JSON state 切到 DB。
- `next <sale-id>` 保留，但基于 purchase / thread / DB 状态给建议。
- `purchase fulfill <purchase-tx>` 不作为主入口；fulfill 应通过 thread 执行，因为一次 VSS 可以覆盖多个 purchase。
- `purchase settle <purchase-tx>` 可以保留为 primitive 调试命令，但 TUI/daemon 应通过 thread 展示和驱动 settle。
- `phase complete-test-flow` 保留到 full-flow 脚本迁移完成，不作为长期公开命令。
- `test-drop-cli-full-flow.sh` 继续作为回归测试入口；迁移期间必须同时支持旧 state 或提供兼容层。

#### 与 `test-drop-cli-full-flow.sh` 的兼容要求

这条脚本已经证明当前全流程闭合，后续重构必须保护它：

- 每次改 `drop-cli` workflow、DB、proof、oracle、settle 相关逻辑，都必须先跑轻量编译和脚本静态检查。
- 未经用户批准，不重新执行会消耗资源的 full-flow。
- 实施 DB 后，脚本应使用独立临时 DB：

```text
DROP_CLI_DB=$RUN_DIR/drop.sqlite
DROP_CLI_STATE_DIR=$RUN_DIR/state
```

- 迁移阶段允许双写：DB 为主，JSON state 作为兼容输出。
- 当 DB 路径完全稳定后，再删除 JSON state 依赖。
- full-flow 脚本最终仍要验证：
  - Walrus blob active。
  - VDD proof 上链。
  - buyer purchase 上链。
  - seller fulfill 上链。
  - Oracle Worker report 上链。
  - seller settle 上链。
  - recovered asset hash 与原文件一致。

2026-06-30 本轮新增功能实施记录：

- 保留 `phase complete-test-flow <sale-id> --yes` 原型兼容路径，未改动其 bridge 逻辑。
- 新增 JSON 轻量 store 作为 SQLite 前的兼容实现：
  - `SaleState` 继续保留在 `DROP_CLI_STATE_DIR/*.json`。
  - `ThreadState` 新增到 `DROP_CLI_STATE_DIR/threads/*.json`。
  - 后续迁移 SQLite 时必须保持 CLI 命令语义不变。
- 新增 `drop-cli db init|migrate|inspect`：
  - 当前初始化 state/thread 目录。
  - `inspect` 输出本地 sale/thread/purchase 数量。
- 新增本地对象发现命令：
  - `drop-cli channel list`
  - `drop-cli channel show <channel>`
  - `drop-cli sale list [--channel <channel>]`
  - `drop-cli sale show <sale-id>`
  - `drop-cli purchase list [--channel <channel>] [--sale <sale-id>] [--status <status>]`
  - `drop-cli purchase show <purchase-tx>`
  - `drop-cli thread list [--channel <channel>] [--sale <sale-id>]`
  - `drop-cli thread show <thread-id>`
  - `drop-cli thread cancel <thread-id>`
- 新增 phase thread 入口：
  - `drop-cli phase respond <purchase-tx>...`
  - `drop-cli phase fulfill <thread-id>`
  - `drop-cli phase settle <thread-id>`
- 当前限制：
  - `purchase list/show` 先基于本地 state 中已记录的 purchase tx，不主动扫全链 purchase event。
  - `phase respond` 可基于本地 purchase tx 自动创建或复用 thread，并输出 thread id。
  - 对已经 settled 的 purchase，thread 会直接显示 `Completed`，并展示 fulfill/settle tx。
  - 2026-07-01 更新：native batch VSS fulfill / native thread settle 已接入手动 phase；仍未接入 daemon 自动策略。
- 已用上次 full-flow run dir 验证：
  - `drop-cli db inspect`
  - `drop-cli sale list`
  - `drop-cli purchase list`
  - `drop-cli phase respond <purchase-tx>`
  - `drop-cli thread list`
  - `drop-cli thread show <thread-id>`
  - `drop-cli phase fulfill <thread-id>` 对 completed thread 幂等。
  - `drop-cli phase settle <thread-id>` 对 completed thread 幂等。
- 本轮未重跑 `test-drop-cli-full-flow.sh` 的原因：
  - 脚本文件没有代码 diff。
  - `phase complete-test-flow` 兼容入口仍存在，且无 `--yes` 时仍只打印 gate，不执行资源消耗动作。
  - 本轮新增的是本地对象发现/thread JSON 管理和幂等展示，没有改 Walrus 上传、链上交易、Prove Network、Oracle Worker 或 recover hash 逻辑。
  - 已执行 `bash -n drop-cli/scripts/test-drop-cli-full-flow.sh` 通过。

2026-06-30 产品化缺口复盘：

当前 `drop-cli` 还不能视为功能全部完成。已跑通的是 prototype full-flow 和本地对象管理雏形，不是完整 seller CLI。

功能当前状态：

- `proof vdd <sale-id> --yes` 已调用 VDD Prove Network flow 并提交 `submitVDDProof`。
- `proof vss <sale-id> --yes` 已对 state 中第一笔 purchase context 执行 VSS fulfill。
- `settle <sale-id> --yes` 已对 state 中第一笔 purchase 执行 settle。
- `recover-test <sale-id>` 已基于 purchase context 和 fulfill tx 执行恢复测试。
- `phase fulfill <thread-id>` 已支持 thread 级 batch VSS：对 `needsVSS=true` 的 purchase 生成一次 batch proof 并调用 `shareDataKey`。
- `phase settle <thread-id>` 已支持 thread 内 purchase 串行 settle；daemon partial success / retry UI 还未产品化。
- `tx resume` 不是真 resume，只提示 status。
- `purchase list/show` 只读取本地 state 已记录的 purchase，不主动扫链上 event 或 subgraph。
- `channel sync/watch` 在设计中存在，但代码未实现。
- TUI 未实现。
- daemon 未实现。
- SQLite 未实现；当前只是 JSON 兼容 store。
- `phase complete-test-flow` 已改为直接调用 drop-script library，不再依赖 bridge binary，但仍是 prototype e2e 入口。

密钥管理不达标项：

- `SELLER_KEY` / `PRIVATE_KEY` 直接从明文 `.env` 读取。
- `ORACLE_WORKER_TOKEN` 直接从明文 `.env` 读取。
- `test-drop-cli-full-flow.sh` 会把 `.env` 复制到 `/tmp/.../drop-cli.env`，包含私钥和 token。
- `ASSET_ENCRYPTION_KEY` 缺失时默认 `[0x22; 32]`。
- `OWNER_SECRET_KEY` 缺失时默认 `[0x11; 32]`。
- 没有 keystore、系统 keyring、硬件钱包、加密配置文件或权限检查。
- 没有 key rotation / per-sale data key 生成策略。
- 没有统一 secret redaction 层。
- `drop-script` 仍有调试输出会打印 `asset_encryption_key`、VSS key 等 secret；`complete-test-flow` library path 仍会经过这条路径。

产品级补齐标准：

- 不再默认使用固定 key。
- 不把私钥/token 复制到 `/tmp`。
- seller key 支持 keystore / wallet / signer abstraction，至少在 CLI 层不再把明文私钥写入生成文件。
- asset key 每个 sale 生成，开发期可明文保存但必须显式 `--dev-insecure-store-keys` 或配置开启。
- SP1 key、oracle token、wallet key 分开管理。
- 所有日志统一做 secret redaction。
- `doctor` 检查配置文件权限，发现 group/world readable secret env 时 warning。
- bridge 迁出或禁止任何 secret debug print。
- 新增功能必须保持 `test-drop-cli-full-flow.sh` 可用，直到 native CLI flow 完全替代 bridge。

#### 架构测试要求

不需要跑 live full-flow，也必须给新架构配测试。

单元测试：

- DB migration 从空库初始化。
- repository CRUD：channel/sale/purchase/thread/tx/warning。
- purchase 状态机：detected/paid/queued/fulfilled/oracle_pending/settle_ready/settled/blocked。
- thread 状态机：collecting/ready/proving_vss/vss_ready/fulfilling/fulfilled/oracle_pending/settle_ready/settling/completed/blocked/failed。
- batch policy：同一 channel/sale 的 purchase 可以进入同一个 thread，不同 sale 默认不能混批。
- policy 判断：哪些 purchase 可以 auto fulfill，哪些必须人工确认。
- warning 生成：余额低、pending tx 超时、oracle 超时、Walrus 过期。
- secret redaction：任何输出不得包含 private key/token。

集成测试：

- mock RPC logs，测试 channel sync 和 purchase discovery。
- mock receipt，测试 pending/confirmed/reverted/replaced。
- mock Oracle Worker，测试 fulfill/report 状态。
- mock Walrus blob status，测试 active/expired/not found。
- 使用临时 SQLite，测试 daemon tick 幂等性。
- TUI 使用 snapshot 或 view-model 测试，不依赖真实终端交互；重点测试 channel -> sale -> purchases -> thread 的展示层级。

回归测试：

- `cargo check -p drop-sdk`
- `cargo check -p drop-cli`
- `cargo check -p drop-script`
- `bash -n drop-cli/scripts/test-drop-cli-full-flow.sh`
- 在用户批准 live test 时，再跑 `test-drop-cli-full-flow.sh`。

验收标准：

- 旧 full-flow 脚本功能不退化。
- TUI 可以按 channel/sale 列出 purchases、threads 和 warnings。
- TUI 可以把多个 purchase 组织成一个 thread，并清楚展示 VSS/fulfill/oracle/settle 进度。
- daemon 可以只读同步 channel 状态并生成 warning。
- daemon 在默认配置下不自动响应 purchase、不自动发 seller 交易、不自动请求证明。
- 开启 `auto_fulfill/auto_settle` 后，仍遵守证明失败停止、tx 幂等、DB lock 和人工确认策略。

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
- 当时曾补充过 `drop-script` bridge binary；该过渡文件后来已删除，功能迁移为 `drop-cli` 直接调用 `drop-script` library。
- 2026-06-29 执行 settlement resume：
  - 已发送 `submitDataKeyCommitment` tx: `0xcc2b81c714320d2f6b50c13684517df69c220099771033bdc1fe6ded15954eaa`
  - 已发送 buyer `purchase` tx: `0xc249bb439c7f28142169723b765b88aa8296afc94e378673ee5673bc5e3c4fd0`
  - 未发送 fulfill/oracle/settle。
  - 停止原因：seller fulfill 前证明阶段失败。VSS Prove Network 返回 `Proof request 0x4e24a6022a2bb5c272dabd3502abc912a46acb07a28bd766ace816eeb364262d is unexecutable`；同次运行中 VDD proof 生成成功，但合约 verifier 模拟返回 `InvalidPublicValues()`。
  - 已修正 `stage_3_fulfill`：VSS proof 或 VSS verifier 模拟失败后立即返回，不再继续请求 VDD proof。
  - 发现状态设计问题：buyer purchase 后 `ephemeral_pubkey` 没有写入 drop-cli state；如果进程在 purchase 后失败，不能安全恢复同一笔 purchase 的 seller fulfill。后续必须把 purchase context 持久化，或者把 purchase/fulfill 设计成单次不中断的原子阶段。

当时未执行：

- 未运行 `asset prepare` 处理大视频，避免触发耗时 Walrus blob id 计算。
- 未通过 `drop-cli proof vss/vdd` 自身提交证明；当前 full-flow 脚本用 `guest/scripts/zk-proof-test.sh` 跑 VSS/VDD Prove Network 和 preflight。
- settlement resume 已执行到 buyer purchase，但未完成 fulfill、Oracle Worker fulfill、settle 和 recover-test 的真实闭环。

当时剩余实施：

- `proof vss/vdd` 需要从门禁提示升级为调用现有 SP1 Prove Network proof flow，并严格防止失败后盲目重试。
- `phase prove/settle/verify` 需要在 proof、Oracle fulfill、settle 实装后串联。
- `tx resume` 需要读取状态中的交易记录并刷新 receipt，而不是仅提示 `drop-cli status`。
- 需要补 purchase context 持久化，至少保存 fulfill 所需的 buyer purchase tx、secret sharing key 派生参数和 ephemeral pubkey。
- 需要排查 VSS Prove Network `unexecutable` 和 VDD verifier `InvalidPublicValues()` 的根因，再由用户批准后重新请求证明。
- 需要补 Oracle Worker fulfill、settle、recover-test 的真实命令实现。

2026-06-30 已补齐原型 full-flow 缺口并完成一次真实全流程测试：

- 明确 `drop-cli` 不承载 buyer 前端 `purchase` 原子命令；当前 full-flow 原型中 buyer purchase 只作为测试流程的一部分，由 `complete-test-flow` 执行。
- 过渡 bridge binary 已删除；`complete-test-flow` 现在直接调用 `drop-script` library，执行 `submitVDDProof -> oracle worker -> buyer purchase -> fulfill -> oracle wait -> settle -> recover`。
- `drop-script` 阶段函数调整：
  - `stage_1_6_submit_vdd_proof` 可被 `drop-cli` library path 调用。
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

### 2026-06-30 合约与配套设施更新

本轮因为 `VSS` 新增了供 CLI / daemon 判断 VSS 复用的辅助 view，重新部署了主合约组件。最终有效部署如下：

- `ExchangeHub`: `0xc857542964E8F7618F1A372c36E180D5670b1669`, block `282682922`
- `ExchangeChannelImplementation`: `0xBAA3089aC201AEc7A33B0DE42C1598Af92d9Fc24`, block `282682879`
- `OracleProxy`: `0xA79E3d31A95eB1368028ba7b25a2B7b8f56146D9`, block `282682863`
- `VSS verifier`: `0x90933a2D8556Bf0785be48D95516238F8C788eBf`
- `VDD verifier`: `0x23e85B3d3dCD4597a40CcDE987ac2BA5c7F3481D`
- `centralizedOracleSigner`: `0x5318831f07e8E5e3e8Fdf2a53ef0F0c3996a88dF`
- `subgraph`: Studio `v0.0.8`, query URL `https://api.studio.thegraph.com/query/1722405/test-arbitrum-store/v0.0.8`

这次部署曾出现一次参数错误：`contracts/.env` 的 `VSS_ADDRESS` / `VDD_ADDRESS` 仍是旧 verifier，导致第一次新 Hub 指向 `0x5e80...` / `0x154D...`。该部署不作为有效版本使用。修正 `contracts/.env` 后重新部署，最终 Hub 已验证指向 `0x9093...` / `0x23e8...`，与 `drop-script/.env`、`contracts/deployed.md`、subgraph manifest 一致。

验证结果：

- `forge test --use /usr/local/bin/solc`: 28 passed。
- `cargo check -p drop-cli`: passed，仅有既有 warning。
- `cargo check -p drop-script`: passed，仅有既有 warning。
- `pnpm --dir subgraph codegen`: passed。
- `pnpm --dir subgraph build`: passed。
- `drop-script/scripts/check-env.sh --section contracts` 在显式加载 `drop-script/.env` 和 `contracts/.env` 后：21 pass，0 warn，0 action required。

注意：默认 `forge test` 会尝试安装 solc `0.8.25`，当前网络下可能超时。本轮使用系统 `solc 0.8.20` 完成编译、测试和最终部署；源码 pragma 均兼容 `^0.8.20`，但 Arbitrum Sepolia RPC 仍会提示 EIP-3855 兼容性 warning。

### 当前 CLI 完成度

当前 `drop-cli` 已经能通过测试脚本完成 prototype full-flow，且底层合约、Oracle Worker、subgraph、VSS/VDD verifier 已重新对齐。但从产品级 CLI 角度还没有全部完成：

- 已完成：底层 publish / prepare / proof / oracle / settle / verify 所需的大部分链路能力。
- 已完成：`phase` 复合命令和 `test-drop-cli-full-flow.sh` 覆盖现有主流程。
- 已完成：`needsVSS` / `isPrivy` 兼容判断，用于区分 buyer 是否需要 VSS。
- 未完成：TUI。
- 未完成：daemon 自动刷新 channel、划分 batch、自动 fulfill / settle。
- 未完成：内置轻量级数据库替代 fixture 文件。
- 未完成：产品级密钥管理，包括加密 keystore、外部 signer、key rotation 和最小权限 profile。
- 已完成：原生 batch VSS 证明和 `shareDataKey` 调度。`phase respond` 可以把多个 `needsVSS=true` 的 purchase 组成 thread，`phase fulfill <thread-id>` 会生成一次 batch VSS proof 并调用 channel `shareDataKey`，`phase settle <thread-id>` 会逐笔 settle。
- 未完成：daemon 自动 batch 策略、长期运行锁、nonce/pending tx 管理，以及更细粒度的 partial success / retry UI。

因此，当前不能把 `drop-cli` 描述为“产品级全部开发完成”；只能描述为“prototype full-flow 可用，核心合约/脚本/Worker 配套已对齐”。

### File Mall 与 Channel 设计匹配性

用户提出的产品假设：

- 第一个应用是 file mall。
- seller 给每一个 list 的文件开一个 channel。
- 因此 channel 内 VSS 复用不重要。
- 以后做订阅频道时，channel 复用才重要。

这个判断与当前合约设计匹配：

- 当前 `ExchangeChannel` 的 `dataKeyCommitment` 和 `privyBitmaps` 是 channel 级状态。
- `listFile` 可以在一个 channel 下登记多个 `saleId -> dataId/version`。
- buyer 一旦在 channel 里完成 VSS 并被标记为 privy，后续同 channel 的 sale 可以复用这一状态。
- 因此一个 file mall 资产一个 channel 时，VSS 复用价值不大，但隔离性好，避免不同文件共享 channel 级 key / privy 状态。
- 订阅频道场景中，一个 channel 下多个 sale 共享访问关系，VSS 复用才有实际价值。

限制也必须明确：如果未来订阅频道要求每个 asset 使用完全独立的数据密钥，而不是共享 channel 级密钥，则当前 channel 级 `dataKeyCommitment` 设计不够，需要把 key commitment / privy 状态下沉到 sale 或 asset 维度。

### 本轮经验

- 重新部署合约前必须同时核对 `contracts/.env`、`drop-script/.env` 和上一轮实际跑通的链上 Hub verifier，不能只看变量名。
- 每次部署后必须立刻读链验证 Hub 的 `implementation`、`oracleWrapper`、`vssVerifier`、`vddVerifier`，以及 OracleProxy 的 `controller`、`centralizedOracleSigner`、`defaultMode`。
- `check-env.sh` 的 contracts 检查需要显式加载包含 RPC 的 env；否则会给出假的 no-code 结果。
- 重任务必须串行执行。本轮曾错误并行启动 `forge test`、`cargo check -p drop-cli`、`cargo check -p drop-script`，后续不能重复。

### 2026-07-01 后续全功能计划

当前 `drop-cli` 已具备单 sale / 单 purchase 的原生 prototype 主流程，但距离产品级全功能还差以下工作。

必须完成：

- Batch VSS：
  - 已完成：将 thread 从“第一笔 purchase”升级为 purchase batch。
  - 已完成：为同一 channel/sale 下多个 `needsVSS=true` 的 purchase 生成一次 batch VSS proof。
  - 已完成：调用 channel `shareDataKey(proof, publicValues, audiences, encryptedDataKeys)` 一次性标记多个 buyer 为 privy。
  - 已完成：对 batch 内每笔 purchase 逐笔 settle；已 privy buyer 走 no-vss 单笔 thread。
  - 未完成：更细粒度的 partial success / retry UI，以及 daemon 自动 batch 策略。
- 长期运行产品级 daemon：
  - 配置文件支持 channel refresh interval、batch window、max batch size、auto_respond、auto_fulfill、auto_settle、spend limits。
  - daemon 长期运行时必须持久化锁，防止多实例同时处理同一 purchase。
  - 交易发送必须串行 nonce 管理，检测 pending/replaced/reverted，避免 nonce 死锁。
  - 默认不自动请求 Prove Network，不自动发 seller 交易；需要显式配置。
  - 周期性 health warning：RPC、Walrus publisher、Oracle Worker、subgraph、seller balance、SP1 requester readiness。
- 状态存储：
  - 当前 JSON store 可继续作为 prototype，但产品级要迁移到 SQLite。
  - 增加 schema version 和 migration。
  - 给 channel/sale/purchase/thread/tx/warning 建索引。
- Buyer purchase context：
  - 合约 `purchase` 不保存 ECIES ephemeral pubkey；首次 VSS fulfill 必须依赖 buyer 前端/链下订单元数据。
  - CLI 需要 `purchase import-context` 或 SDK API，接收 buyer 前端传来的 fulfill 所需元数据。
  - 没有 purchase context 时，CLI 必须清楚提示无法为首次 buyer 做 VSS fulfill。
- TUI：
  - 当前 `drop-cli tui` 只是只读 dashboard。
  - 产品级 TUI 需要 channel -> sale -> purchase -> thread 分层视图、选择 purchase、创建 batch、触发 fulfill/settle、显示 warnings。
- Subgraph 集成：
  - `purchase list` 需要能从 subgraph/链上事件发现 purchase，而不是只读取本地 state。
  - subgraph 不可用时 fallback 到 RPC log scan。
- 密钥管理：
  - 当前仍读取明文 `.env`。
  - 产品级需要 keystore/keyring/external signer 支持、secret redaction、禁止把 secret 写入 `/tmp` 生成文件。
- 测试：
  - 单元测试覆盖 config/state/thread/batch policy。
  - mock RPC/worker 测 daemon one-shot。
  - Arbitrum Sepolia full-flow 只在明确批准后运行。

执行顺序：

1. 先实现 purchase context import 和 exchangeInfo 持久化。
2. 已完成：实现 batch thread 状态机的手动路径。
3. 已完成：实现 batch VSS proof + `shareDataKey`。
4. 未完成：实现每笔 purchase 的 daemon partial success / retry 策略。
5. 将 JSON store 迁移到 SQLite。
6. 实现 daemon run loop、锁、nonce/pending tx 管理。
7. 实现 TUI 交互层。
8. 最后跑全流程和回归测试。
