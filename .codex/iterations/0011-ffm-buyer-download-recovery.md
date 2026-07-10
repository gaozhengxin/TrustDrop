# 迭代: FFM Buyer 下载与恢复闭环

## 背景

Fair File Marketplace 当前已经具备 buyer 侧基础 marketplace 原型：

- 从 subgraph 拉取 listed sales。
- 首页推荐、浏览、搜索、标签过滤和详情页。
- 钱包插件连接。
- 调用合约 `purchase`。
- 本地 IndexedDB 保存 buyer purchase thread 初始记录。
- 0 号内容引擎规则过滤旧 sale。

最近一次链上调试证明，frontend purchase 发出后，seller 可以用 `drop-cli` 找到 purchase、fulfill 并 settle：

- purchase tx: `0xbf1821868690821427b392f91698ece069c34b5bc1a24c422156f60255ae88be`
- fulfill tx: `0x26eb50fd8991e4d6249b9975bcaba3d26b89c5c3cc799a7e38ccb7e17c7c8a5c`
- settle tx: `0x7ac9454bf8fcf3f1a048e0106d92de27dd4b5bf43201ce6ec3f40bcc99aaf04a`

当前缺口是 buyer 前端还不能完成“购买后下载并恢复原文件”的最后闭环。

## 目标

本迭代目标是把 FFM buyer 前端从可购买原型推进到可下载恢复的可用版本：

1. Buyer 能在购买记录中看到购买、fulfill、settle、可下载等协议状态。
2. Buyer 能从 Walrus aggregator 下载加密 blob。
3. Buyer 能用钱包派生出的本地 secret sharing key 恢复 asset encryption key。
4. Buyer 能解密文件并在浏览器中下载原文件。
5. Walrus aggregator 能在前端配置，内置两到三个可选公共 Mainnet aggregator，并允许自定义 URL。
6. 前端不依赖 seller 本地状态或 drop-cli fixture，全部通过钱包、本地 buyer thread、subgraph、链上事件和 Walrus aggregator 完成。

## 范围

本轮包含：

- `packages/drop-ts-sdk` buyer recover/download 能力。
- `app/gui` 下载恢复 UI。
- Buyer thread 状态机增强。
- Walrus aggregator 配置、预设、健康检查和失败回退。
- subgraph/RPC 链上状态查询兜底设计。
- Buyer 本地资产管理面板。
- 前端和 TS SDK 测试。

本轮不包含：

- Seller fulfill/settle 自动化，seller 仍使用 `drop-cli`。
- 新合约功能。
- 新 guest 程序或证明逻辑。
- Walrus publisher 上传功能，前端 buyer 只读 Walrus blob。
- Storail 后端内容引擎。
- Tauri 或移动端支持。

## 实施方法

### 1. Walrus aggregator 配置

前端需要支持一个明确的 aggregator 配置模型：

- 默认使用 Mainnet aggregator。
- 提供预设下拉选择。
- 支持输入自定义 aggregator URL。
- 配置保存在 localStorage 或 IndexedDB。
- 下载前对选中 endpoint 请求 `/v1/api` 或小型 blob 状态请求做健康检查。
- 下载 blob 使用稳定路径：`GET <aggregator>/v1/blobs/<blobId>`。
- 如果当前 aggregator 状态不可用，直接显示不可用状态，并提示用户到 Settings 手动切换；首版不自动 fallback。

当前预设候选来自 Walrus 官方 Network Reference 和 `operators.json`。实现前需要再次打开官方列表确认仍有效。

初始预设建议：

| 名称 | URL | 说明 |
| --- | --- | --- |
| Mysten Labs | `https://aggregator.walrus-mainnet.walrus.space` | 官方参考 Mainnet aggregator |
| H2O Nodes | `https://aggregator.walrus-mainnet.h2o-nodes.com` | 官方 operator list 中靠前且带 cache 的 Mainnet aggregator |
| Studio Mirai | `https://aggregator.mainnet.walrus.mirai.cloud` | 官方 operator list 中靠前且带 cache 的 Mainnet aggregator |

注意：

- 不要硬编码唯一 aggregator。Walrus 官方文档明确提示 community endpoint 会变化。
- Mainnet 没有公开无认证 publisher，本迭代不做前端上传。
- 文件下载和解密必须闭环，否则不算完成产品功能。
- 大文件下载要考虑浏览器内存压力，首版可以先支持当前约 100MB Apollo 文件，后续再推进 streaming/chunk decrypt。

### 2. Buyer thread 与资产状态

FFM 不是网盘客户端，不追踪“用户是否已经保存文件”这类文件管理状态。前端只需要管理购买资产的协议状态和一次性下载/解密操作反馈。

现有 thread 只有 purchase 后的本地记录。本轮需要增加协议状态：

- `purchase_seen`: 前端已拿到 purchase tx。
- `purchase_indexed`: subgraph 已索引 purchase。
- `fulfilled`: 已发现 seller fulfill/shareDataKey。
- `settled`: 已发现 settle。
- `ready_to_download`: 已拿到恢复所需链上事件和 Walrus blob 信息。

下载/解密是用户触发的一次性操作状态，只用于当前操作 UI 和错误提示：

- `downloading`: 正在从 aggregator 下载密文。
- `decrypting`: 正在恢复 asset key 并解密。
- `failed`: 某一步失败，保留可重试信息。

状态更新来源：

- IndexedDB 本地 thread。
- subgraph buyer activity，用于 purchase、fulfill、settle 等链上事实。
- RPC receipt/log 兜底，用于 subgraph 未及时同步或缺字段时的链上事实恢复。
- sale listing 数据中的 `walrusBlobId`、`encryptedBlobId`、`dataVersion`、file metadata。

内容规则只影响 marketplace 浏览、搜索和推荐。用户已经购买的资产不受内容规则过滤影响，必须继续出现在资产管理面板中。

### 3. TS SDK 模块划分

`packages/drop-ts-sdk` 增加或扩展模块：

- `walrus.ts`
  - aggregator preset 定义。
  - URL normalize。
  - health check。
  - blob download。
- `recover.ts`
  - 解析 fulfill/settle 所需事件。
  - 派生 buyer secret sharing key。
  - 支持用户手动提交 secret/private material 作为高级恢复路径。
  - 解密 wrapped asset key。
  - 解密 asset ciphertext。
- `threads.ts`
  - thread 状态枚举扩充。
  - 状态合并与迁移。
  - 错误和 retry metadata。
- `contracts.ts` 或 `events.ts`
  - 合约 event ABI 和 receipt/log 解析。

关键要求：

- 默认 buyer secret sharing key 由 buyer 钱包签名派生，派生输入必须与 `purchase.ts` 保持一致。
- Purchase 时提供高级选项，让用户选择密钥管理模式；默认使用确定性派生规则。
- 资产管理面板必须记录每个资产使用的密钥模式和恢复能力：如果当前状态可以解密，用户点击下载时直接解密；如果不能解密，用户点击下载时要求用户提交对应私钥或 secret。
- 前端不能要求 seller 导出任何本地文件。
- 解密后的文件名、content type 和 size 优先来自 subgraph sale metadata。

### 4. 前端页面与交互

需要新增或完善页面：

- Records 列表：显示每笔购买的状态和下一步。
- Purchase detail/thread detail：显示 sale、purchase tx、fulfill tx、settle tx、Walrus blob、aggregator、错误信息和操作按钮。
- Asset management panel：显示用户购买的全部资产，不受内容规则过滤；显示链上状态、恢复能力、下载状态、失败状态。
- Download panel：显示 aggregator 状态、下载进度、解密进度、下载按钮；aggregator 不在此处配置，只提示去 Settings 切换。下载完成后触发浏览器保存，不在应用内长期记录“已保存”状态。
- Settings：Walrus aggregator 独立配置入口，可选预设和自定义 URL。

首版不做 preview。资产面板需要根据 content type 或文件名展示文件图标，至少区分：

- data
- text
- image
- video
- audio/music
- binary/program
- unknown

页面上不要使用解释性原型文案。状态和错误信息只用于操作反馈。

### 5. Subgraph 与 RPC 协同

subgraph 不是下载数据库，也不是 buyer 本地资产状态数据库。它只负责提供链上事实的索引视图：

- listed sales。
- buyer purchases。
- settlements/refunds。
- fulfill/shareDataKey 相关事件。

本轮判断 purchase、fulfill、settle 状态时优先使用 subgraph；如果 subgraph 同步延迟或缺字段，先用 RPC receipt/log 兜底。缺字段本身属于索引设计问题，需要记录为后续 subgraph schema/mapping 改进，但不应该把 download/decrypt 这种本地动作塞进 subgraph。

RPC 兜底范围：

- 用户刚完成 purchase/subgraph 未同步。
- seller 刚 fulfill/subgraph 未同步。
- seller 刚 settle/subgraph 未同步。
- 单笔 tx receipt/log 解析。

download/decrypt 状态只属于 buyer 本地资产管理：

- `ready_to_download`
- `downloading`
- `decrypting`
- `failed`

其中 `ready_to_download` 可作为购买资产的持久协议状态；`downloading`、`decrypting`、`failed` 只作为当前操作反馈或最近一次错误记录，不把应用做成网盘客户端，也不追踪用户是否已经保存文件。subgraph 只告诉前端链上交易是否走到可恢复条件，前端再用本地密钥状态和 Walrus aggregator 完成下载解密。

## 研究笔记

Walrus 官方 Network Reference 说明：

- Aggregator 通过 HTTP 读取 blob。
- Mainnet 参考 aggregator 是 `https://aggregator.walrus-mainnet.walrus.space`。
- 稳定读取路径是 `GET $AGGREGATOR/v1/blobs/<BLOB_ID>`。
- 官方维护 `operators.json`，community endpoint 会变化。
- 不应在生产代码中硬编码单个 community endpoint。
- Mainnet 没有公开无认证 publisher。

这和本项目需求匹配：FFM buyer 前端只需要读取 Walrus blob，不需要上传；seller 侧上传仍由 `drop-cli` 和 seller 的 Walrus publisher 节点完成。

## 测试验收标准

实施完成后至少满足：

- `pnpm --dir app/gui build` 通过。
- `pnpm --dir packages/drop-ts-sdk build` 或等价 typecheck 通过。
- aggregator preset normalize/selection/health check 有单元测试。
- buyer key derivation 与 purchase 逻辑复用或测试证明一致。
- 用当前 Apollo sale 的已完成购买记录，前端能识别 settled 状态。
- 前端能从配置的 Walrus aggregator 下载密文 blob。
- 前端能解密并导出原文件，文件大小与 sale metadata 匹配。
- aggregator 失败时不会破坏 thread 状态，用户可以切换 endpoint 后重试。
- 已购买资产即使被内容规则过滤，也仍在资产管理面板中显示。
- 每笔重复购买都显示，不合并隐藏。
- 失败状态保留最近一次失败阶段、错误信息和时间，支持用户重试下载/解密。
- 不追踪“用户是否已经保存文件”，不做网盘式文件管理。

## 推进计划

第一步：设计确认

- 确认本迭代范围。
- 确认 aggregator 预设列表。
- 首版必须完成下载和解密闭环；允许先采用整文件下载/解密，后续再升级 streaming。

第二步：SDK 基础能力

- 增加 Walrus aggregator preset/config/download 模块。
- 增加 recover/event 解析模块。
- 扩展 buyer thread 状态机。
- 增加 buyer asset/key management 数据结构。
- 加单元测试。

第三步：前端状态和页面

- Settings 增加 aggregator 配置。
- Records/detail/asset management 页面接入 thread 状态机。
- Purchase 完成后自动进入可恢复 thread。
- 下载面板接入 SDK。
- 文件图标按 content type/文件名分类显示。

第四步：恢复闭环

- 根据 fulfill/settle 事件找到 encrypted data key。
- 由 buyer 钱包派生 secret sharing key。
- 解密 asset key。
- 下载密文 blob。
- 解密原文件并触发浏览器下载。

第五步：验收与记录

- 用当前 Apollo sale 的 purchase/fulfill/settle 记录做一次手动验收。
- 记录所有 tx、blob id、aggregator URL 和结果。
- 更新本迭代经验总结。

## 经验总结

### 2026-07-07 实施进展

已完成首轮实现：

- `packages/drop-ts-sdk/src/walrus.ts`
  - Walrus Mainnet aggregator preset。
  - aggregator URL normalize、localStorage 保存、健康检查、blob 下载。
  - 32-byte Walrus blob id hex 到 URL-safe base64 blob id 的转换。
- `packages/drop-ts-sdk/src/recover.ts`
  - buyer 钱包派生恢复 secret。
  - 根据 `DataKeyShare` 找到 buyer 的 encrypted data key。
  - ChaCha8 解开 asset key。
  - 从 Walrus aggregator 下载密文 blob。
  - 使用 `trustdrop_asset_v1` nonce 解密原文件并按 metadata size 截断。
- `packages/drop-ts-sdk/src/subgraph.ts`
  - buyer activity 查询增加 `dataKeyShares`。
- `app/gui`
  - 增加 Settings 页面管理 aggregator。
  - Records 改为 buyer 资产管理视角，已购资产不再受内容规则过滤。
  - 资产行显示文件类型 badge。
  - settled/ready asset 提供 Download 操作。
  - 下载失败只作为最近一次操作错误，不做网盘式保存状态。

验证：

- `pnpm --dir app/gui build` 通过。
- Walrus blob id 编码离线验证通过：`0x021b943dff92a73b9a980c6e689382fb18685e75325582b62ee9689e30a4acbb` 转换为 `AhuUPf-SpzuamAxuaJOC-xhoXnUyVYK2LulonjCkrLs`。

剩余风险：

- 尚未在浏览器中实际点击 Download 做端到端下载解密验收。
- 当前大文件恢复首版采用整文件下载/解密，后续需要 streaming/chunk decrypt。
- `dataKeyShares(first: 100)` 是首版足够用的查询，后续如果 channel 交易量变大，需要按 channel/block/buyer 优化查询或补 subgraph 反向索引。

### 2026-07-08 修复记录

问题：

- 前端下载时所有预设 Walrus aggregator 都返回 HTTP 404。
- 根因不是 aggregator 不可用，而是前端使用了错误的下载 blob id。
- 旧实现把 `Sale.dataCommitment` 转成 Walrus blob id；但 `dataCommitment` 是原文 asset id，不是密文 blob id。

修复：

- `TrustDropSubgraph.listBuyerActivity` 增加 `vddProofs` 查询。
- `recoverPurchasedAsset` 下载目标改为：
  1. 优先使用 listing metadata 中显式 `walrusBlobId` / `blobId`。
  2. 否则使用同 channel 最新 `VddProof.cCipher` 转成 Walrus blob id。
  3. 如果两者都没有，明确报错要求等待 VDD proof indexing 或刷新 activity。
- 不再用 `dataCommitment` 推导 Walrus 下载目标。

验证：

- `pnpm --dir app/gui build` 通过。

### Cloudflare Pages 部署

FFM frontend 使用 Cloudflare Pages 部署，构建产物目录是 `app/gui/dist`。

部署脚本：

```bash
scripts/deploy-ffm-cloudflare-pages.sh
```

默认配置：

- project: `fair-file-marketplace`
- branch: `dev`

可用环境变量覆盖：

```bash
CLOUDFLARE_PAGES_PROJECT=<project-name>
CLOUDFLARE_PAGES_BRANCH=<branch-name>
```

脚本复用 `oracle-worker` 包中已安装的 `wrangler`，避免前端包重复维护 Cloudflare CLI 依赖。

2026-07-09 首次部署：

- project: `fair-file-marketplace`
- branch: `dev`
- preview URL: `https://1391461b.fair-file-marketplace.pages.dev`
