# 迭代: Chainlink Functions 迁移到 CRE

## 日期

2026-06-23

## 背景

0005 当前暂停。本插队任务研究 Chainlink Functions sunset 对 TrustDrop 的影响，并制定迁移到 Chainlink CRE 的计划。

用户提供的信息：

- 当前 Chainlink Functions 即将 sunset，需要迁移到 CRE。
- 参考文档：`https://docs.chain.link/cre/reference/clf-migration-ts`。
- 当前测试网络是 Arbitrum Sepolia。
- 地址 `0x9396532cc74858e8d9be52df5f5f682B272AcB89` 有 Chainlink 余额。

官方文档关键信息：

- Chainlink Functions testnet sunset 日期是 2026-06-15，mainnet shutdown 文档语境中是 2026-09-01；文档也提示 2026-06-30 及临时服务中断窗口。迁移应按紧急项处理。
- CRE 迁移模型不是“换 router 地址”。CLF 的路径是 consumer 调 `_sendRequest()`，DON 回调 `fulfillRequest()`；CRE 的路径是 workflow 通过 trigger 启动，生成 signed report，再通过 `KeystoneForwarder` 调用实现 `IReceiver.onReport` 的合约。
- CRE 支持 Arbitrum Sepolia。官方 Supported Networks 表列出 Arbitrum Sepolia，最低版本：CLI `v1.0.0+`、Go SDK `v1.0.0+`、TS SDK `v1.0.1+`。
- Arbitrum Sepolia 的 CRE chain name 是 `ethereum-testnet-sepolia-arbitrum-1`，Forwarder 地址是 `0x76c9cf548b4179F8901cda1f8623568b58215E62`。
- CRE workflow deployment 目前需要 Early Access；需要运行 `cre account access` 或访问 `app.chain.link/cre/request-access` 申请。等待期间可以先本地 simulate。

参考来源：

- Chainlink CLF -> CRE migration TS: `https://docs.chain.link/cre/reference/clf-migration-ts`
- CRE Supported Networks: `https://docs.chain.link/cre/supported-networks-ts`
- CRE Forwarder Directory: `https://docs.chain.link/cre/guides/workflow/using-evm-client/forwarder-directory-ts`
- CRE Building Consumer Contracts: `https://docs.chain.link/cre/guides/workflow/using-evm-client/onchain-write/building-consumer-contracts`
- CRE Deploying Workflows: `https://docs.chain.link/cre/guides/operations/deploying-workflows`

## 目标

- 判断当前 TrustDrop 合约是否需要适配 CRE。
- 给出推荐迁移架构。
- 明确哪些步骤 Codex 可以操作，哪些步骤必须用户手动处理。
- 给出从当前 Chainlink Functions 到 CRE 的实施计划。
- 本文档完成后等待用户确认，再实施合约/脚本/CRE workflow 改动。

## 范围

本轮只做研究和计划。

允许研究：

- `contracts/src/oracle/OracleProxy.sol`
- `contracts/src/oracle/FunctionsConsumer_Walrus.sol`
- `contracts/src/VDD.sol`
- `contracts/src/ExchangeChannel.sol`
- `contracts/script/DeployMain.s.sol`
- Chainlink CRE 官方文档

本轮不实施：

- 不修改合约。
- 不部署合约。
- 不安装 CRE CLI。
- 不创建 CRE workflow 项目。
- 不调用 Chainlink 控制台或 Early Access 流程。
- 不使用用户地址签名。
- 不发交易。

## 当前 TrustDrop Oracle 结构

当前路径：

```text
ExchangeChannel.fulfill
  -> VDD.submitVDDProof
  -> VDD._triggerOracle(cCipher)
  -> OracleProxy.request(cCipher, address(this))
  -> WalrusFunctionsConsumer.executeRequest(args, subscriptionId)
  -> Chainlink Functions DON 执行 inline JavaScript
  -> WalrusFunctionsConsumer.fulfillRequest(requestId, response, err)
  -> OracleProxy.handleResponse(requestId, response, err)
  -> VDD.onResponse(cCipher, response)
  -> oracleSuccessUntil[cCipher] = endTime
```

关键合约：

- `OracleProxy.sol`
  - 保存 `consumer`、`subscriptionId`、`controller`。
  - `request(bytes c_cipher, address callback)` 调用 `IFunctionsConsumer.executeRequest`。
  - `handleResponse(bytes32 requestId, bytes response, bytes err)` 只允许 `consumer` 回调。
- `WalrusFunctionsConsumer.sol`
  - 继承 `FunctionsClient`。
  - 内嵌 Chainlink Functions JavaScript source。
  - 使用 `donID`、`gasLimit`、`apiKey`、`subscriptionId`。
  - `fulfillRequest` 回调 `OracleProxy.handleResponse`。
- `VDD.sol`
  - 只依赖 `IOracleProxy.request(cCipher, address(this))`。
  - 回调入口是 `onResponse(bytes cCipher, bytes response)` 和 `onOracleError` 预期。
  - 不直接依赖 Chainlink Functions 类型。

## 是否需要调整合约

结论：需要调整 Oracle 层合约，不能只换配置。

原因：

- CRE 的 onchain write 不会调用当前 `WalrusFunctionsConsumer.fulfillRequest`。
- CRE workflow 会把 signed report 交给 `KeystoneForwarder`，再由 forwarder 调用目标 receiver 的 `onReport(bytes metadata, bytes report)`。
- 当前 `OracleProxy.handleResponse` 只允许 `consumer` 地址调用，而 CRE 回调调用方应是 KeystoneForwarder。
- 当前 `OracleProxy.request` 主动同步调用 `consumer.executeRequest` 获取 `requestId`；CRE 更适合由 EVM Log trigger 监听链上事件启动 workflow。
- 当前 inline JavaScript source 需要迁移为 CRE TypeScript workflow，不能继续作为 `FunctionsRequest` source 字符串使用。

可以尽量不改：

- `VDD.sol` 的 `IOracleProxy.request(cCipher, address(this))` 调用方式可以保留。
- `VDD.onResponse(cCipher, response)` 的 response ABI 可以保留为当前 64 字节编码 `(uint256 status, uint256 endTime)`。
- `ExchangeChannel.fulfill / settle` 的外围逻辑可以保持不变。

必须替换或新增：

- 用 `OracleProxyCRE` 替换当前 `OracleProxy`。
- 移除 `WalrusFunctionsConsumer` 作为运行时依赖；可保留源码做历史参考。
- 新增 CRE workflow TypeScript 项目，负责监听 Oracle 请求事件、查询 Walrus API、生成 report 并写回 receiver。
- 部署新的 OracleProxy/Hub，或者如果后续设计允许可用可升级/adapter，但当前部署不是可升级结构，实际链上生效需要重新部署主合约。

## 推荐迁移架构

推荐保持 `IOracleProxy` 接口不变，重写 OracleProxy 内部机制：

```text
VDD._triggerOracle(cCipher)
  -> OracleProxyCRE.request(cCipher, callback)
       - require whitelist
       - requestId = keccak256(chainid, address(this), callback, cCipher, nonce)
       - requests[requestId] = { cid: cCipher, client: callback }
       - emit RequestSent(requestId, callback, cCipher)

CRE workflow
  -> EVM Log trigger 监听 OracleProxyCRE.RequestSent
  -> 从 event 读取 requestId、callback、cCipher
  -> 把 cCipher bytes32 转 Walrus BlobId base64url
  -> HTTP GET Blockberry/Walrus mainnet API
  -> 计算 status/endTime
  -> payload = abi.encode(requestId, status, endTime, errBytes)
  -> runtime.report(payload)
  -> evm.writeReport({ receiver: OracleProxyCRE, report })

KeystoneForwarder
  -> OracleProxyCRE.onReport(metadata, report)
       - ReceiverTemplate 验证 forwarder
       - decode payload
       - 找 requests[requestId]
       - response = abi.encode(status, endTime)
       - 调 ctx.client.onResponse(ctx.cid, response)
       - delete requests[requestId]
```

推荐 payload：

```solidity
abi.encode(
  bytes32 requestId,
  uint256 status,
  uint256 endTime,
  bytes err
)
```

其中：

- `status == 2`: Walrus blob active/ensured，`endTime` 为按 Walrus epoch 推导的到期时间。
- `status == 1`: 可检索但不能证明覆盖足够长窗口时，复用当前 VDD 逻辑给 `GRACE_PERIOD`。
- `status == 0`: 不可检索或 API 失败。
- `err.length > 0`: 可选错误路径，触发 `onOracleError(bytes,bytes)`；没有必要时可始终传空 bytes 并用 `status=0` 表示失败。

安全要求：

- `OracleProxyCRE` 必须只接受官方 KeystoneForwarder 调用。Arbitrum Sepolia forwarder：`0x76c9cf548b4179F8901cda1f8623568b58215E62`。
- `OracleProxyCRE` 应限制允许的 workflow 身份。仅检查 forwarder 不够，因为同一 forwarder 可能转发多个 workflow。应根据 CRE metadata 中的 workflow id / owner / name 做 allowlist，或使用 Chainlink `ReceiverTemplate` 推荐的权限方式。
- `requestId` 必须不可预测冲突，并绑定 `cCipher`、callback、nonce、chainid 和 proxy 地址。
- `onReport` 必须检查 request 存在，处理后删除，防止重复 report。
- `endTime` 仍保留当前 VDD 的上限检查：不能超过 `block.timestamp + 10 * 365 days`。
- CRE workflow 中不能使用 `Date.now()`；应按官方要求使用 `runtime.now()` 或确定性时间来源。

## 对当前合约的具体影响

### `OracleProxy.sol`

建议替换为 `OracleProxyCRE.sol`：

- 删除 `IFunctionsConsumer`、`consumer`、`subscriptionId`。
- 增加 `keystoneForwarder` 或使用 `ReceiverTemplate(forwarder)`。
- 保留：
  - `controller`
  - `whiteList`
  - `RequestContext`
  - `RequestSent`
  - `CallbackResult`
  - `request(bytes,address)`
  - `setWhitelist(address,bool)`
- 新增：
  - `uint256 public nonce`
  - `mapping(bytes32 => RequestContext) public requests`
  - `onReport(bytes metadata, bytes report)` / `_onReport(...)`
  - workflow allowlist 配置

### `FunctionsConsumer_Walrus.sol`

迁移后不再部署。

其中 inline JS 逻辑需要迁移到 CRE TypeScript：

- `hexToBase64Url`
- Blockberry API URL
- `x-api-key`
- Walrus epoch 到 timestamp 的换算
- status 编码

### `VDD.sol`

理论上可不改。

需要检查：

- `onResponse` 期望 `response.length == 64`，所以 CRE receiver 写回时必须继续传 `abi.encode(uint256 status, uint256 endTime)`。
- `onFail` 当前只把 `oracleSuccessUntil[cCipher] = 0`，失败后仍可再次触发 Oracle，保留即可。

### `ExchangeChannel.sol`

理论上可不改。

`fulfill` 中 `_triggerOracle(vdd.cCipher)` 的异步模型仍成立。

### `DeployMain.s.sol`

需要改：

- 不再读取 `CL_SUB_ID`、`CL_ROUTER`。
- 新增读取 `CRE_FORWARDER`，Arbitrum Sepolia 默认可用 `0x76c9cf548b4179F8901cda1f8623568b58215E62`。
- 部署 `OracleProxyCRE`。
- 不再部署 `WalrusFunctionsConsumer`。
- 部署后配置 workflow allowlist。

## CRE Workflow 设计

建议新增目录：

```text
cre/walrus-availability/
  project.yaml
  workflow.yaml
  config.json
  secrets.yaml
  package.json
  src/main.ts
  src/abi.ts
```

配置建议：

```json
{
  "chainName": "ethereum-testnet-sepolia-arbitrum-1",
  "chainSelector": "<按 CRE SDK 示例或生成配置填写>",
  "oracleProxy": "<OracleProxyCRE address>",
  "receiver": "<OracleProxyCRE address>",
  "forwarder": "0x76c9cf548b4179F8901cda1f8623568b58215E62",
  "walrusApiBase": "https://api.blockberry.one/walrus-mainnet/v1/blobs"
}
```

secrets：

```yaml
secrets:
  - id: BLOCKBERRY_API_KEY
```

workflow 逻辑：

1. 注册 EVM Log trigger，监听 `OracleProxyCRE.RequestSent(bytes32 indexed requestId,address indexed client,bytes cid)`。
2. 解码 event。
3. 校验 `cid` 长度为 32 bytes。
4. 把 hex blob id 转 base64url。
5. 调用 Blockberry Walrus mainnet API。
6. 用 `runtime.now()` 计算当前 epoch，不使用 `Date.now()`。
7. ABI encode `(requestId, status, endTime, err)`.
8. `runtime.report(payload)`.
9. `evm.writeReport({ receiver: oracleProxy, report, gasLimit })`.

## 是否支持 Arbitrum Sepolia

支持。

官方 Supported Networks 明确列出 Arbitrum Sepolia：

- Network: `Arbitrum Sepolia`
- CRE chain name: `ethereum-testnet-sepolia-arbitrum-1`
- CLI: `v1.0.0+`
- Go SDK: `v1.0.0+`
- TypeScript SDK: `v1.0.1+`

Forwarder Directory 明确列出：

- Arbitrum Sepolia forwarder: `0x76c9cf548b4179F8901cda1f8623568b58215E62`

## Codex 可以操作的部分

在用户批准后，Codex 可以做：

- 新增 `OracleProxyCRE.sol`。
- 新增 CRE workflow 项目骨架。
- 把 `FunctionsConsumer_Walrus.sol` 的 JS 逻辑迁移为 TypeScript workflow。
- 更新 `DeployMain.s.sol`。
- 更新合约测试，覆盖：
  - `request` 生成 requestId 并保存 context。
  - 非 forwarder 不能 `onReport`。
  - 未授权 workflow 不能更新状态。
  - 正常 report 会调用 VDD/onResponse 并写入 `oracleSuccessUntil`。
  - 重放同一 requestId 失败或无效。
- 本地编译：
  - `forge build`
  - `forge test`
- 如果 CRE CLI 可安装，可执行本地 `cre workflow simulate`。
- 更新 `.codex/docs/contracts.md`、`.codex/docs/drop-script.md` 和 0005 checklist 中 Oracle 部分。

## 需要用户手动处理的部分

用户必须处理或确认：

- 申请 CRE Early Access：
  - 运行 `cre account access`，或访问 `https://app.chain.link/cre/request-access`。
- 安装并登录 CRE CLI：
  - 按官方文档安装 `cre`。
  - `cre login` 或官方当前认证命令。
- 决定 workflow registry：
  - 测试阶段推荐 private registry。
  - public/onchain registry 需要 linked wallet 和 Ethereum Mainnet gas；测试阶段不建议先走 public registry。
- 重新注册 secret：
  - CLF DON-hosted secret 不能导出，需要用 CRE secret 流程重新创建 `BLOCKBERRY_API_KEY`。
- 使用地址 `0x9396532cc74858e8d9be52df5f5f682B272AcB89`：
  - 可作为 CRE linked EVM key 或部署/管理相关地址。
  - 需要用户确认该地址可用于 CRE 账户绑定和签名。
  - 该地址已有 Chainlink 余额，但 CRE private registry 的 workflow 管理不一定需要链上 LINK；Arbitrum Sepolia 合约部署仍需要 ETH。
- 确认 Blockberry/Walrus API key 可用。
- 确认迁移完成前是否保留旧 Functions 部署作为临时 fallback。

## 实施计划

### 阶段 1: 设计定稿

- 用户确认是否采用“保留 `IOracleProxy`，替换内部为 CRE receiver”的方案。
- 用户确认是否接受重新部署 Hub/Channel/Oracle。
- 用户确认 workflow trigger 使用 EVM Log trigger，而不是 HTTP trigger 或 cron。

验收：

- 本文档通过用户确认。

### 阶段 2: 合约适配

- 新增 `OracleProxyCRE.sol`。
- 增加最小 `IReceiver`/ReceiverTemplate 依赖。
- 保留 `request(bytes,address)` 外部接口。
- `request` 改为 emit event，不再调用 Functions consumer。
- `onReport` 解码 CRE report 并调用 VDD `onResponse`。
- 增加 workflow 身份 allowlist。

验收：

- `forge build` 通过。
- `forge test` 通过。
- 新增 Oracle CRE 单元测试通过。

### 阶段 3: CRE workflow 本地项目

- 新建 `cre/walrus-availability`。
- 实现 EVM Log trigger。
- 实现 Walrus mainnet API 查询和 status/endTime 计算。
- 实现 report 写回。
- 本地 simulation。

验收：

- `cre workflow simulate` 能用样例 event 生成正确 payload。
- 不能依赖 Node-only API。
- 不使用 `Date.now()`，改用 CRE runtime 时间。

### 阶段 4: 测试网部署

- 部署新 OracleProxyCRE + Hub/Channel。
- 更新 `drop-script/.env`、`contracts/deployed.md`、subgraph manifest。
- 部署或激活 CRE workflow。
- 发起一次 VDD proof 后确认：
  - `RequestSent` 事件出现。
  - CRE workflow 执行。
  - `OracleProxyCRE.onReport` 被 forwarder 调用。
  - `oracleSuccessUntil[cCipher]` 更新。

验收：

- `drop-script` 能从 fulfill 走到 `oracleSuccessUntil` 更新。
- settle 不再卡在 Chainlink Functions。

### 阶段 5: 清理旧 CLF 路径

- 停止部署 `WalrusFunctionsConsumer`。
- 清理或归档旧 Functions 文档。
- 更新 0005 checklist：把 Chainlink Functions subscription 检查替换为 CRE workflow / forwarder / secret / deployment status 检查。

## 风险和待确认问题

- CRE workflow deployment 需要 Early Access，Codex 不能代替用户完成账户申请。
- CRE metadata 中 workflow 身份的 Solidity 解析方式需要按 `ReceiverTemplate` 实际实现确认；不要只检查 forwarder。
- 当前 `cCipher` bytes 到 Walrus BlobId base64url 的编码逻辑仍是关键风险，迁移时必须写单元测试固定。
- Blockberry API key 当前在合约 storage 中明文保存；迁移到 CRE 后应放入 CRE Vault secret，合约不再保存 API key。
- 如果选择 public/onchain registry，会引入 Ethereum Mainnet linked key 和 gas 需求；测试阶段建议先 private registry。
- 当前 `OracleProxy.setWhitelist` 由 Hub/controller 管理，重新部署后必须确认 channel/VDD 仍能触发 request。

## 测试验收标准

本研究阶段：

- [x] 确认 CRE 支持 Arbitrum Sepolia。
- [x] 确认 Arbitrum Sepolia CRE forwarder 地址。
- [x] 确认当前合约不能无改动适配 CRE。
- [x] 给出合约改造计划。
- [x] 给出 CRE workflow 计划。
- [x] 区分 Codex 可操作步骤和用户手动步骤。

后续实施阶段：

- [ ] `forge build` 通过。
- [ ] Oracle CRE 单元测试通过。
- [ ] CRE workflow simulation 通过。
- [ ] Arbitrum Sepolia 新部署完成。
- [ ] CRE deployed workflow 成功回写 `oracleSuccessUntil`。
- [ ] `drop-script` 全流程通过 Oracle 阶段。

## 经验总结

- CRE 迁移是结构性迁移，不是 Chainlink Functions router/subscription 参数替换。
- TrustDrop 当前设计里 VDD 与 OracleProxy 有良好隔离，可以把变更控制在 Oracle 层和部署脚本。
- 迁移后 API key 应从合约 storage 移出，进入 CRE secret 管理，这是安全性提升。
- Arbitrum Sepolia 被 CRE 官方支持，技术上可继续保持当前测试网。
