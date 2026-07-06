# 迭代: File Mall Web App

## 日期

2026-07-02

## 背景

TrustDrop 后续前端分为三块：

- Portal 网站。
- 文件商店。
- 文档站。

本轮只做文件商店。文件商店是 buyer 使用的 Web App，不支持桌面、移动原生或 Tauri。当前 `app/gui` 是 Tauri/Vite 默认示例，和目标产品不匹配；本轮实施时可以删除或重建为纯 Web App。

本轮在实施前必须先完成产品设计、架构设计和前置可行性检查。任何业务代码、前端工程、SDK、subgraph schema 或合约改动，都需要在用户确认本设计后再实施。

## 产品名称

确定名称：

```text
Trusted File Marketplace
```

定位是极简资源下载站。名字直接提示三件事：标的是 file/data，形态是 marketplace，平台同时服务 seller 上架和 buyer 购买，交易交付由 TrustDrop 协议提供可信保证。页面重点是浏览、搜索、购买、下载和查看购买记录。

## 目标

- 设计 buyer 端文件商店 Web App。
- 设计 buyer 使用流程和页面布局。
- 设计 browser TypeScript SDK，把 buyer 必要功能从 App 中抽离。
- 设计 App 内部 TrustDrop 协议 lib，封装 subgraph 查询和产品级组合逻辑。
- 检查当前 subgraph 是否足以作为商城数据库。
- 检查 buyer TS SDK 所需功能的可行性。
- 明确本轮实施范围、风险和验收标准。

## 范围

允许在用户确认后修改：

- `app/gui` 或重建为新的纯 Web App。
- 新增 TypeScript SDK 包。
- App 内部 TrustDrop lib。
- subgraph schema、mapping、README 和部署脚本。
- 前端相关 package 配置。

本轮不默认修改：

- VSS/VDD guest 程序。
- 合约核心协议逻辑。
- Oracle Worker fulfill 逻辑。
- Rust `drop-cli` 主流程。
- Rust `drop-script` 主流程。

如前端所需数据无法从当前 subgraph 获得，优先修改 subgraph；只有 subgraph 无法表达的链上事实，才考虑合约事件补充。

## Buyer 产品流程

### 1. 进入首页

首页展示：

- 推荐文件。
- 最新上架。
- 热门成交。
- 搜索入口。
- 分类/tag 快捷入口。
- 钱包连接状态。

首页推荐算法第一版不做复杂模型，使用 subgraph 查询结果在前端组合：

- 最近上架且仍 active。
- 购买次数高。
- 成交次数高。
- tag 与用户最近浏览或购买记录相近。
- 价格在合理范围内。

### 2. 浏览商城

商城页展示 asset 列表：

- 标题。
- 简短描述。
- tags。
- 价格。
- seller/channel。
- 上架时间。
- 购买次数。
- 成交次数。
- 状态。

需要支持：

- 分页。
- 排序。
- tag 筛选。
- 时间筛选。
- 价格区间筛选。
- 搜索关键词。

### 3. 搜索商品

搜索页/搜索栏支持：

- 按标题、描述、tag 搜索。
- tag name 模糊匹配。
- 按时间范围过滤。
- 按购买次数/成交次数排序。

如果 subgraph 不能做足够的模糊搜索，第一版允许：

- subgraph 返回候选集；
- 前端用 GraphQL 查询结果做二次过滤；
- 当数据量变大时再引入专门搜索服务。

### 4. 查看商品详情

详情页展示：

- 文件名称和描述。
- tags。
- 文件大小。
- 价格。
- seller/channel 信息。
- Walrus blob 状态。
- 是否已购买。
- 购买按钮。
- 协议状态说明。

详情页不展示复杂密码学细节，但需要提供可验证状态：

- sale 是否 active。
- data commitment / version。
- VDD 是否已通过链上验证。
- Oracle/Walrus 可用性状态。

### 5. 购买

buyer 连接钱包后：

1. 生成或读取 buyer 本地密钥。
2. 发起 `purchase` 交易。
3. 本地记录 purchase tx。
4. 等待 seller fulfill。
5. 监听 subgraph 或链上事件。
6. 在 fulfill 后恢复 data key。
7. 下载 Walrus encrypted blob。
8. 解密并还原文件。
9. 支持保存文件。

购买动作属于 buyer 前端功能，不进入 seller `drop-cli`。

### 6. 购买记录

记录页展示：

- pending purchase。
- fulfilled purchase。
- 可下载 purchase。
- 已 settle purchase。
- refund/expired purchase。

每条记录要能进入详情，展示：

- purchase tx。
- sale/channel。
- 当前状态。
- 下一步动作。
- 文件恢复状态。
- 下载入口。

### 7. 本地状态与 thread

Buyer 端也需要 thread 概念，但和 seller CLI 不同：

- 一个 buyer purchase 对应一个 buyer-side thread。
- thread 记录 purchase tx、seller fulfill 状态、key recovery 状态、download/decrypt 状态。
- thread 存在浏览器本地轻量数据库中。

第一版可以使用 IndexedDB，不使用后端数据库。

## 页面布局

整体风格：

- 极简。
- 低装饰。
- 高信息密度但不拥挤。
- 不使用复杂渐变、大 hero、营销页结构。
- 所有页面优先服务重复使用和状态检查。

建议结构：

```text
Top nav:
  Trusted File Marketplace | Browse | Search | Records | Wallet

Home:
  Search input
  Recommended assets
  Latest assets
  Popular assets

Browse:
  Left filters
  Main asset table/list
  Sort + pagination

Asset Detail:
  Asset metadata
  Seller/channel
  Protocol status
  Purchase panel

Records:
  Purchase status tabs
  Purchase/thread list
  Recovery/download actions

Settings:
  Wallet
  Buyer key status
  RPC/subgraph endpoint
  Local data controls
```

### 当前设计原型保留

当前 `app/gui` 中已经实现的页面作为设计原型保留：

- Home: 推荐资源、最新上架、搜索入口。
- Browse: tag 筛选、搜索、资源列表。
- Asset Detail: 资源详情、协议状态摘要、购买入口占位。
- Records: buyer purchase thread 记录占位。

这些页面当前使用 mock data，不连接 subgraph，不发链上交易。后续开发不能直接删除这些原型，而应逐步把 mock data 替换为 TS SDK / subgraph / wallet / IndexedDB 的真实数据源。

2026-07-03 更新：上述页面已从 mock data 切换为真实逻辑骨架：

- Sale/listing 数据来自 subgraph `v0.0.10`。
- Wallet 连接使用浏览器注入钱包和 Arbitrum Sepolia。
- Records 按连接的钱包地址查询 subgraph purchase / settlement / refund。
- 本地 buyer thread 存储使用 IndexedDB。
- Purchase key package 使用 wallet signature 派生 `secret_sharing_key`，读取 channel `ownerPublicKey`，并生成 ECIES encrypted VSS key、VSS key commitment 和 ephemeral public key。

当前仍未直接提交 purchase 交易。原因是当前合约 `purchase` 只保存 `encryptedVssKey`，没有把 ECIES `ephemeral_pubkey` 写入链上事件或 subgraph；而 seller fulfill 必须拿到这个 ephemeral public key 才能解开 buyer 的 secret sharing key。现有 CLI 依赖本地 buyer context 传递该字段。Trusted File Marketplace 需要补一个 seller 可读的 ephemeral key 传递通道后，再开放真实 purchase submit。

### 需要补齐的子页面

Buyer 主流程页面：

- Search Results: 独立搜索结果页，展示关键词、筛选条件、排序、空状态。
- Asset Detail / Purchase: 购买确认页，展示价格、chain、seller、data commitment、deadline、buyer key 派生状态。
- Purchase Submitted: purchase tx 已提交页，展示 tx hash、确认状态、下一步等待 seller fulfill。
- Purchase Thread Detail: 单个 purchase thread 的完整状态页，展示 purchase、fulfill、oracle、recovery、settle/refund 的阶段进度。
- Recovery: 文件恢复页，展示 key recovery、Walrus download、decrypt、truncate、save file 的进度和错误。
- Download Complete: 恢复成功页，展示文件名、大小、校验状态、保存入口。
- Refund / Expired: 交易过期或 seller 未 fulfill 时的退款页。

Buyer 账户与本地状态页面：

- Wallet: 钱包连接、chain 切换、地址状态。
- Buyer Key: 钱包签名派生 buyer data seed 的说明、签名入口、缓存状态、清除缓存。
- Local Threads: IndexedDB 中所有本地 thread 的维护页，包括导出诊断信息、清理失败记录。
- Settings: RPC、subgraph endpoint、oracle worker endpoint、Walrus aggregator endpoint。

Marketplace 浏览页面：

- Category / Tag Detail: 单个 tag/category 的资源列表、成交趋势。
- Seller Profile / Channel: seller/channel 的资源列表、成交历史、可信状态。
- Collection / Bundle: 如果 metadata 支持多个文件或资源包，展示 bundle 内文件。
- Trending: 按购买次数、成交次数、最近活跃排序的榜单页。

Seller 可见页面：

- Seller Landing: 面向 seller 的简短入口，说明上架需要使用 `drop-cli`。
- Seller Channel Preview: 从 subgraph 展示某 seller/channel 已上架的资源和状态。

系统状态页面：

- Protocol Status: 当前 hub、subgraph、oracle worker、Walrus、chain id 的只读状态。
- Not Found: sale/channel/thread 不存在。
- Error Detail: GraphQL、RPC、wallet、Walrus、decrypt 等错误的诊断页。

## 前端工程建议

用户已确认：重建现有 `app/gui` 为纯 Web App，删除原有 Tauri 目录。

```text
app/gui/
```

候选技术栈：

- Vite。
- TypeScript。
- viem。
- IndexedDB wrapper。
- 简单 CSS 或很薄的组件层。

不使用 Tauri。当前 `app/gui/src-tauri` 在实施阶段删除。

当前第一版脚手架使用 Vite + Vanilla TypeScript，避免在产品结构尚未稳定时引入额外 UI 框架。后续如果页面状态复杂度明显上升，再迁移到 React。

## TypeScript SDK 设计

已新增独立包：

```text
packages/drop-ts-sdk/
```

包名建议：

```text
@trustdrop/sdk
```

这个 SDK 面向浏览器 buyer，不替代 Rust `drop-sdk`。Rust `drop-sdk` 继续服务 Rust script/CLI。

当前实现模块：

- `config`: Arbitrum Sepolia chain id、RPC、subgraph endpoint。
- `subgraph`: sale、buyer purchase、settlement、refund 查询。
- `wallet`: injected wallet 连接和 chain 切换。
- `threads`: IndexedDB buyer thread 存储。
- `purchase`: buyer key package 准备逻辑。

### SDK 模块

#### `chain`

功能：

- 创建 viem public client / wallet client。
- 校验 chain id，目前目标是 Arbitrum Sepolia。
- 读取合约地址配置。
- 查询 receipt、logs、block timestamp。

#### `contracts`

功能：

- `purchase` 调用封装。
- `settle/refund` 只读状态判断。
- 查询 channel / sale 基础状态。
- 查询 `vddVerified`、`oracleSuccessUntil`、`isPrivy` 等状态。
- 编码/解码事件。

#### `proof`

功能：

- Buyer 主路径不在浏览器本地重新验证 SP1 Groth16 proof。
- 通过链上 verifier 调用结果、channel 状态、receipt 和 public values 绑定关系进行轻客户端验证。
- 提供 proof status normalizer：
  - `vddVerified`。
  - `dataKeyShared`。
  - `settleReady`。
  - `oracleValidUntil`。

说明：

- 完整本地 SP1 Groth16 验证不作为第一版浏览器 SDK 功能。
- 如果将来要本地验证，需要单独研究 verifier wasm、proof artifact 获取和性能。

#### `crypto`

功能：

- Buyer wallet key 由钱包插件管理，Web App 不接触钱包私钥。
- Buyer data key / secret sharing key 通过钱包签名导出。
- ECIES 或当前协议所需的 key unwrap。
- ChaCha8 解密。
- 文件恢复。

注意：

- 钱包私钥不能进入 App、URL、日志、subgraph、localStorage 或 IndexedDB。
- 浏览器端通过 domain-separated wallet signature 派生 buyer-side data seed，再按 sale/original asset id 派生 `secret_sharing_key`。
- 派生消息必须包含 app 名、chain id、channel、sale id、data commitment/version 和用途，避免跨站/跨链/跨 sale 复用。
- 派生出的 buyer data seed 可以用 IndexedDB + WebCrypto 包装缓存；也可以每次通过钱包签名重新生成。
- 当前 Rust 参考逻辑是 `drop_lib::kdf::key_derive(ecc_sk, msg_hash) = sha256(ecc_sk || msg_hash)`；浏览器不能取得 `ecc_sk`，所以 TS SDK 应替换为 `sha256(wallet_signature || domain_context)` 或更标准的 HKDF 形式。
- 当前购买流程中 `secret_sharing_key` 被 ECIES 加密给 seller，并用 `blake3(secret_sharing_key)` 作为 VSS commitment；seller fulfill 后 buyer 使用该 key 解开 asset key。

#### `walrus`

功能：

- 下载 encrypted blob。
- 查询 blob 可用性。
- 对接 Oracle Worker blob status API。
- 计算或校验 blob id，如浏览器可行。

#### `subgraph`

功能：

- GraphQL client。
- asset 查询。
- purchase 查询。
- settlement/refund 查询。
- channel 查询。
- 首页推荐查询。

#### `thread`

功能：

- Buyer purchase thread 状态机。
- IndexedDB 持久化。
- 恢复 pending purchase。
- 引导下一步动作。

状态示例：

```text
created
purchase_submitted
purchase_confirmed
waiting_fulfill
fulfilled
key_recovered
file_downloaded
file_restored
settled
refunded
failed
```

## App 内部 TrustDrop Lib

App 内部 lib 不重复 SDK 底层能力，而是做产品级组合：

```text
app/file-mall/src/lib/trustdrop/
```

建议职责：

- asset card view model。
- asset detail view model。
- buyer dashboard state。
- 推荐算法。
- 搜索参数到 GraphQL query 的转换。
- purchase thread 到 UI 状态的转换。
- 错误文案和下一步动作。

## Subgraph 前置检查

### 当前状态

已执行：

```sh
pnpm --dir subgraph build
```

结果：

```text
Build completed: build/subgraph.yaml
```

构建通过，且没有产生未提交 diff。

2026-07-03 已增强并部署 marketplace subgraph：

```text
Studio version: v0.0.10
Query URL: https://api.studio.thegraph.com/query/1722405/test-arbitrum-store/v0.0.10
ExchangeHub: 0x4845b28ae7e3e558A445a1A03ACD07d7c55976d1
Start block: 283665140
OracleProxy: 0x824C886cBDD933751A04D4Fc1e8a97f771Bf2FE6
ExchangeChannelImplementation: 0x32EBB1F6D5729Fd6a4700f62542c64d4B348640D
```

已执行：

```sh
pnpm --dir subgraph codegen
pnpm --dir subgraph build
SUBGRAPH_VERSION_LABEL=v0.0.10 pnpm --dir subgraph deploy:studio
SUBGRAPH_QUERY_URL=https://api.studio.thegraph.com/query/1722405/test-arbitrum-store/v0.0.10 pnpm --dir subgraph check:marketplace
```

检查结果：

```text
12/12 checks passed
basic asset query: 0 rows
frontend recommendation inputs: 0 candidate rows
```

新版 schema、查询能力和当前 endpoint 均已通过。v0.0.10 从新 Hub 起扫，当前没有新 sale，因此 asset query 返回 0 rows 是预期状态。

当前 schema 包含：

- `ExchangeChannel`
- `Sale`
- `Tag`
- `Purchase`
- `Settlement`
- `Refund`
- `Audience`
- `DataKeyCommitment`
- `DataKeyShare`
- `VddProof`
- `OracleRequestSkipped`

### 基础 asset 查询能力

当前可行，已由 `check:marketplace` 验证。

`Sale` 已包含：

- `channel`
- `saleId`
- `dataCommitment`
- `price`
- `version`
- `info`
- `title`
- `description`
- `fileName`
- `fileSize`
- `contentType`
- `tags`
- `normalizedTags`
- `purchaseCount`
- `settlementCount`
- `refundCount`
- `lastPurchasedAt`
- `lastSettledAt`
- `status`
- `listedAtTimestamp`
- `updatedAtTimestamp`

限制：

- 当前没有单独 `Asset` 实体，`Sale` 作为 marketplace listing。
- 旧上架流程的 `info` 可能仍是普通字符串；只有 seller 上架时把 `info` 写成约定 JSON，subgraph 才能填充标题、描述、文件名、大小、content type 和 tags。

建议：

- 第一版继续使用 `Sale` 作为 asset listing。
- `info` 统一约定为 JSON metadata。

### 按 tag name 模糊查询

当前部分支持。

原因：

- schema 已有 `tags`、`normalizedTags` 和 `Tag` 实体。
- 支持 exact tag / normalized tag 查询和 tag 聚合。
- The Graph 不适合做通用模糊文本搜索。

可选方案：

1. 当前方案：subgraph 返回候选 sale/tag，前端对 `title`、`description`、`tags`、`normalizedTags` 做二次 fuzzy match。
2. 如果数据量变大：引入独立搜索服务或 The Graph full-text/search 特性专项评估。
3. 当前不做：在 subgraph mapping 中实现复杂分词或编辑距离搜索。

### 按时间筛选

当前可行。

字段：

- `listedAtTimestamp`
- `updatedAtTimestamp`
- `Purchase.timestamp`
- `Settlement.timestamp`
- `Refund.timestamp`

GraphQL 可按 timestamp 做区间过滤和排序。

### 按购买次数/成交次数排序/筛选

当前可行，已由 `check:marketplace` 验证。

当前 `Sale` 维护：

- `purchaseCount`
- `settlementCount`
- `refundCount`
- `lastPurchasedAt`
- `lastSettledAt`
- `lastRefundedAt`

`ExchangeChannel` 也维护 sale/purchase/settlement/refund 聚合计数，`Tag` 维护 sale/purchase/settlement 聚合计数。

### 前端推荐算法

当前可做简版。

简版输入：

- listedAtTimestamp。
- price。
- status。
- purchase count。
- settlement count。
- tags。
- buyer 本地浏览/购买历史。

仍有妥协：

- 如果当前链上 `info` 不是 JSON，metadata 字段会回退为空或原始 `info` 字符串。
- 模糊搜索和个性化推荐在前端完成，不在 subgraph 内完成。

建议第一版推荐：

```text
score =
  recencyWeight
  + purchaseCountWeight
  + settlementCountWeight
  + tagAffinityWeight
  - stalePenalty
```

推荐算法在 App 内部 lib 中实现，不放进 TS SDK。

## Buyer TS SDK 可行性检查

### 验证证明

可行，但第一版定义为链上轻验证：

- 查询 VDD/VSS 相关合约状态。
- 检查 fulfill/settle 交易 receipt。
- 检查 `DataKeyShared`、`VDDProofSubmitted` 等事件。
- 检查 public values 与 sale data commitment / data version 的绑定关系，如果前端能取得 proof calldata。

不建议第一版做：

- 浏览器本地 SP1 Groth16 proof 验证。
- 从 Prove Network 拉 proof artifact 后本地复验。

### 操作合约

可行。

buyer 必需写操作主要是：

- `purchase`。
- 可能的 `refund`。
- 可能的 buyer-side `settle` 不应由 buyer 执行，当前合约 `settle` 是 seller owner 调用路径；buyer 端主要等待 seller settle 或在超时后 refund。

只读操作：

- 查询 sale/channel。
- 查询 pending exchange 状态。
- 查询 buyer 是否已 privy。
- 查询 oracle/vdd 状态。

需要从当前 ABI 生成或维护 TypeScript ABI。

### 还原文件

可行，但需要把 Rust 当前恢复流程迁移到 TS：

- 下载 Walrus encrypted blob。
- 从 DataKeyShared 事件中找到 buyer 对应 encrypted data key。
- 使用 buyer 本地密钥恢复 asset key。
- 使用 ChaCha8 解密。
- 按 `original_len` truncate。
- 浏览器保存文件。

风险：

- 当前 `original_len` 不在 subgraph 结构化字段中，可能需要放入 `info` JSON 并解析。
- 文件大小上限按 RSLH/VE 当前覆盖窗口控制为 `228124672` bytes。
- 浏览器内存会承载 encrypted blob 和 decrypted blob，第一版应限制文件大小。

### 密钥管理

可行，但必须改成钱包插件优先。

第一版要求：

- 钱包插件管理 buyer wallet key。
- App 通过 `personal_sign` 或等价签名能力导出 buyer data seed。
- 每个 purchase 的 `secret_sharing_key` 从 buyer data seed + sale context 派生。
- IndexedDB 只保存 thread 状态、交易状态、可选的加密缓存，不保存钱包私钥。
- UI 明确提示换钱包或签名不可复现会导致无法恢复历史购买。

不允许：

- 钱包私钥写入 App 存储。
- buyer data seed 明文写入 subgraph。
- 私钥写入 URL。
- 私钥打印到 console。
- 私钥明文保存在普通 localStorage。

### Thread 管理

可行。

buyer thread 来源不是用户手动创建，而是 purchase 交易确认后自动创建。thread 负责恢复购买流程：

- purchase tx。
- sale/channel。
- fulfill 等待。
- key share 检测。
- blob 下载。
- decrypt/recover。
- settle/refund 结果。

第一版持久化使用 IndexedDB。

## Subgraph 需要增强的 schema 候选

建议增强 `Sale`：

```graphql
type Sale @entity(immutable: false) {
  id: Bytes!
  channel: Bytes!
  saleId: Bytes!
  dataCommitment: Bytes!
  price: BigInt!
  version: Bytes!
  info: String!
  title: String
  description: String
  fileName: String
  fileSize: BigInt
  contentType: String
  tags: [String!]!
  normalizedTags: [String!]!
  purchaseCount: BigInt!
  settlementCount: BigInt!
  refundCount: BigInt!
  lastPurchasedAt: BigInt
  lastSettledAt: BigInt
  status: String!
  listedAtTimestamp: BigInt!
  updatedAtTimestamp: BigInt!
}
```

可选新增：

```graphql
type Tag @entity(immutable: false) {
  id: String!
  name: String!
  normalizedName: String!
  saleCount: BigInt!
  purchaseCount: BigInt!
  settlementCount: BigInt!
}
```

## 实施方法

等待用户确认后，建议按以下顺序实施：

1. 增强 subgraph schema/mapping，使商城查询具备数据库能力。
2. 增加 TypeScript SDK 包，先做只读 GraphQL、链上 read、buyer key/thread 基础。
3. 重建纯 Web App，不保留 Tauri。
4. 实现浏览、搜索、详情、记录四个核心模块。
5. 接入 purchase 写交易和 buyer thread。
6. 接入文件恢复路径。
7. 增加推荐算法。
8. 补测试和构建脚本。

不建议一开始做复杂 UI。先把数据、状态和交易闭合。

## 测试验收标准

设计确认后的第一阶段验收：

- subgraph build 通过。
- TS SDK typecheck 通过。
- App build 通过。
- 首页能查询并展示 active sales。
- Browse 能分页、排序、基础过滤。
- Search 能按 title/description/tag 搜索。
- Records 能从 subgraph + IndexedDB 展示 buyer purchase threads。
- Wallet 能连接并显示当前 chain。
- Purchase 能构造并提交 Arbitrum Sepolia 交易。
- Recovery 能在已有 fulfilled purchase 上恢复文件。

暂不要求：

- 部署 production 前端。
- 支持主网。
- 支持 Tauri。
- 支持本地 SP1 proof 验证。
- 支持正式搜索引擎。

## 研究笔记

- 当前 `app/gui` 是 Tauri 示例，不是可继续演进的文件商城；用户已确认重建 `app/gui` 并删除 Tauri 目录。
- 当前 `subgraph build` 通过。
- 当前 subgraph 适合作为链上事件数据库基础，但还不满足商城查询需要。
- 当前 Rust `drop-sdk` 无法直接作为浏览器 SDK 使用，需要新增 TypeScript SDK。
- 当前 Rust `drop-sdk::proof` 仍是 placeholder，前端不要依赖它。
- Buyer 的 proof verification 第一版应定义为链上轻验证，而不是浏览器本地完整 proof verification。
- 文件大小产品限制应沿用 RSLH/VE 文档：`228124672` bytes。
- Rust 参考密钥路径：
  - `drop-lib/src/kdf.rs`: `key_derive(ecc_sk, msg_hash)` 当前是 `sha256(ecc_sk || msg_hash)`。
  - `drop-lib/src/ecies.rs`: buyer 用 seller VSS public key 加密 `secret_sharing_key`。
  - `drop-script/src/main.rs`: `stage_2_purchase` 当前用测试种子派生 `secret_sharing_key`，并把 ECIES ciphertext 提交到 `purchase`。
  - `drop-script/src/main.rs`: `stage_4_recovery` 使用 `secret_sharing_key` 解开 `DataKeyShared` 里的 asset key，再用 asset key 解密 Walrus blob。
- 浏览器实现不能使用测试种子或导出钱包私钥，应改为 wallet signature KDF。

## 待用户确认

- 产品名已确定为 `Trusted File Marketplace`。
- 用户已确认重建 `app/gui` 为纯 Web App，并删除 Tauri 目录。
- 用户已确认新增 `packages/drop-ts-sdk`。
- 用户已确认先增强 subgraph schema/mapping。
- 用户已确认其它决策项。
- 待最终确认：`info` 统一约定为 JSON metadata 的具体字段。

## 经验总结

待本轮结束后补充。
