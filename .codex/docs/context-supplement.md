# Context 补充知识

## 目的

本文件补充现有文档没有集中说明、但新 context 必须知道的项目知识。它不是替代架构文档或 runbook，而是记录容易被遗漏的状态差异、术语映射、外部服务边界和调试判断原则。

## 新 Context 必须先知道的状态差异

### 本地源码不等于当前链上部署

当前本地源码已经包含 0002 的合约修复：

- `ExchangeChannel.purchase` 强制 `getDataId(dataCommitment) == dataVersion`。
- `ExchangeChannel.purchase` 限制 deadline 为 1 小时到 30 天。

但当前 Arbitrum Sepolia 已部署合约暂不更新。后续集成测试继续使用当前部署版本；如果之后需要重部署，才把这批源码改动一起部署上去。

因此调试时必须明确：

- `forge test` 验证的是本地源码。
- `drop-script` 连接的是 `.env` 中的链上地址。
- 链上合约是否包含本地修复，取决于是否重新部署。

### 已提交和未提交状态

0002 已提交：

- commit `e3cf21f chore: close drop script protocol review`
- 包含 0002 协议审查文档、purchase 修复、事件解析修复、合约测试。

0002 之后新增的调试依赖 runbook 目前是后续补充文档，需要单独提交：

- `.codex/runbooks/drop-script-integration-dependency-runbook.md`

本文档也是后续补充文档。

## 地址与私钥知识边界

文档可以记录地址、部署位置和 env 文件路径，但不能记录私钥、API key、deploy key。

### 已知开发地址

以下是从本地 env 私钥推导出的公开地址，可以记录：

| 用途 | 地址 |
| --- | --- |
| drop-script seller / contracts deploy key 之一 | `0x97502930463A46dC98f97f00f9C02C9A60f15117` |
| drop-script buyer / contracts deploy key 之一 | `0x9396532CC74858E8d9Be52dF5F5f682B272AcB89` |
| SP1 prover / funding address | `0xB34Cdac031d3bF18e014f8e9ce17DDA9cdb9EbE9` |

不要在文档、commit message 或终端输出摘要中写私钥值。

### 私密文件位置

这些文件可以读取用于本地操作，但不能提交：

- `drop-script/.env`
- `contracts/.env`
- `subgraph/.env`

如果新 context 需要部署或发交易，必须先确认用户允许使用这些 env 中的凭证。

## 容易混淆的术语映射

### `dataCommitment`

在当前流程中，`dataCommitment` 是原始明文资产的承诺字节。`drop-script` 当前使用：

- `original_asset_id = compute_rs_id(padded_plaintext)`
- purchase/listing 里的 `dataCommitment = original_asset_id`

链上 `dataVersion` 是：

- `getDataId(dataCommitment)`
- 当前实现等于 `keccak256(dataCommitment)`

不要把 `dataCommitment` 和 Walrus 密文 blob id 混用。

### `cCipher`

`cCipher` 是密文资产的 Walrus BlobId 原始 32 字节表示。当前流程中：

- `drop-script` 计算 `encrypted_blob_id = compute_rs_id(encrypted_asset_data)`
- VDD guest public values 第三段是 `c_cipher`
- 合约 `vddVerified[cCipher]` 和 `oracleSuccessUntil[cCipher]` 都以它为 key

Oracle 当前风险点：

- Solidity 里 `OracleProxy.request` 把 `bytes cCipher` 直接转 `string`。
- Consumer JS 按 hex string 解释参数。
- 正确方向应是显式 bytes-to-hex 后再传给 consumer。

### `walrus_blob_id`

`walrus_blob_id` 是上传到 Walrus publisher 后返回的字符串形式 blob id，用于下载/查询。它和 `encrypted_blob_id: [u8; 32]` 表示同一类对象，但一个是运行时存储接口字符串，一个是证明/合约使用的 32 字节原始值。

调试时要确认：

- `walrus_blob_id` 能被本地 Walrus client 下载。
- `encrypted_blob_id` 和 VDD proof 的 `c_cipher` 一致。
- Oracle 查询的 blob id 编码能指向同一个对象。

### `saleId` 与 `dataVersion`

`saleId` 标识销售条目。

`dataVersion` 标识当前 sale 对应的数据版本。

注意：`getNextSaleId()` 必须在 `listFile` 前读取，作为本次 listing 的 sale id。`listFile` 会递增 nonce，之后再读到的是下一次 sale id。

### `ExchangeInfo`

`ExchangeInfo` 是 purchase digest 的核心输入，包含：

- `saleDigest`
- `price`
- `initTime`
- `deadline`
- `dataCommitment`
- `vssKeyCommitment`

`fulfill`、`settle`、`refund` 都依赖同一份 `ExchangeInfo` 重新计算 digest。脚本解析 purchase event 时必须精确匹配 channel 和 saleId，不能随便取 receipt 中第一条 Hub log。

## 端到端调试的状态机判断

### purchase 成功只代表资金锁定

purchase 成功后：

- buyer 资金锁定在 channel。
- seller 还没有权利 settle。
- buyer 也不能立即拿回资金，除非 deadline 过期后 refund。

### fulfill 成功不代表可以 settle

fulfill 成功后通常表示：

- VSS proof 已提交，buyer 可成为 privy。
- VDD proof 已提交，`vddVerified[cCipher] = true`。
- Oracle 请求已触发或被 cooldown 跳过。

但 settle 还需要：

- `oracleSuccessUntil[cCipher] > initTime + LIVING_WINDOW`

所以 `fulfill` 后必须等 Oracle 回调。当前 0007 运行路径是 centralized Oracle Worker 主动读取链上请求并调用 `OracleProxy.submitCentralizedReport`；CRE-compatible `onReport` 分支保留但暂不使用。

### subgraph 不是安全来源

subgraph 只用于观察事件。结算判断必须以链上状态为准：

- `pendingExchanges[digest]`
- `isPrivy(buyer)`
- `vddVerified[cCipher]`
- `oracleSuccessUntil[cCipher]`
- `lockedBalances(buyer)`

## 外部服务实际边界

### Walrus

本地工作目录：

```text
/home/justin/walrus
```

启动脚本：

```text
/home/justin/walrus/start.sh
```

当前 drop-script 默认 endpoint：

```text
http://localhost:31415
```

新 context 不应假设 Walrus 已启动。每次调试前都要确认。

### SP1 Prover Network

项目曾从 SP1 5.0.8 迁移到 v6 系列。SP1 / Prover Network 更新很快，不保证向后兼容。

任何涉及 SP1 的调试都要确认：

- `sp1-sdk`
- `sp1-zkvm`
- `sp1-build`
- `sp1-lib`
- verifier 合约版本
- Prover Network 当前要求版本

如果 guest 重新编译导致 VK 变化，对应 verifier 必须重新部署。

### Hybrid Oracle / Worker

Oracle 是当前结构闭合中最容易被低估的外部依赖。

必须人工确认：

- centralized Oracle Worker 已部署并配置专用 signer。
- `OracleProxy.centralizedOracleSigner()` 已设置为 Worker signer。
- Worker signer 有足够 Arbitrum Sepolia ETH。
- Worker status 页面 ready，且不暴露具体余额或 secret。
- `OracleProxy.controller()` 指向当前 Hub。
- OracleProxy whitelist 允许 VDD/channel 发 request。
- Walrus / Blockberry API key 可用，但不写入 git。

如果 `fulfill` 成功但一直无法 settle，优先查 Oracle 链路。

### The Graph Studio

subgraph 项目：

```text
https://thegraph.com/studio/subgraph/test-arbitrum-store/
```

subgraph deploy key 在 `subgraph/.env`，不能提交。

合约重新部署后，subgraph 至少要更新：

- Hub 地址
- startBlock
- ABI，如果事件或结构变化
- version label / query URL 记录

## 已知工程债

这些问题不一定阻止单次调试，但新 context 必须知道：

- `stage_4_recovery` 仍按历史 `DataKeyShared` 事件恢复，单次调试可用，多订单并发不安全。
- `drop-script` 仍使用演示固定密钥 `[0x11; 32]` 和 `[0x22; 32]`。
- listing price 目前没有链上强约束，purchase price 可以由 buyer 传入；如果要固定标价，需要后续修改合约。
- channel 当前更像单全局 data key 模型，不是多资产独立密钥模型。
- `settle` 没有 `nonReentrant`，当前状态删除在转账前，直接重入风险较低，但后续应加。
- 固定 `call{gas: 10_000}` 可能不兼容合约钱包。
- Oracle error callback 没有在 VDD 中形成明确失败状态。

## 新 Context 推荐读取顺序

最小读取顺序：

1. `.codex/README.md`
2. `.codex/docs/context-supplement.md`
3. `.codex/docs/project-overview.md`
4. `.codex/docs/architecture.md`
5. `.codex/docs/drop-script.md`
6. `.codex/docs/drop-script-debug-plan.md`
7. `.codex/runbooks/drop-script-integration-dependency-runbook.md`
8. `.codex/iterations/0002-drop-script-protocol-review.md`

如果要部署或调合约，再读：

9. `.codex/docs/contracts.md`
10. `.codex/docs/operations.md`

如果要跨模块改代码，再读：

11. `.codex/docs/modules.md`

## 新 Context 的第一句话建议

如果用户重开 context，可以直接让 LLM 先读：

```text
请先阅读 .codex/README.md、.codex/docs/context-supplement.md、.codex/docs/drop-script-debug-plan.md、.codex/runbooks/drop-script-integration-dependency-runbook.md 和 .codex/iterations/0002-drop-script-protocol-review.md，然后总结当前状态和下一步人工清单，不要先改代码。
```
