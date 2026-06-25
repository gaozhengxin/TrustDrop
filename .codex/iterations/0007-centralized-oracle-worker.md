# 迭代: 中心化 Oracle Worker 与 CRE 兼容合约

## 日期

2026-06-23

## 背景

0006 研究得出：Chainlink Functions 迁移到 CRE 不是简单换配置，当前 TrustDrop Oracle 层需要改造。用户进一步决定：

- 合约要和 CRE 做到兼容。
- 当前不直接使用 CRE，先改用中心化预言机。
- 中心化预言机用 Cloudflare Worker 实现。
- Worker 复用当前 Chainlink Functions 中的 Walrus availability 检查逻辑。
- Worker 由用户主动触发，用户传入链上请求交易。
- Worker 获取交易执行结果，检查合约地址和 logs，解析 oracle 脚本所需参数。
- Worker 执行 oracle 脚本后，模仿 CRE 执行逻辑，触发链上交易。
- Worker 免费运行，但只处理 TrustDrop 自己的请求。
- `drop-script` 要加入触发 oracle 的动作；未来应下沉到 `drop-sdk`。
- 未来合约提供两个分支：中心化 oracle 和 Chainlink/CRE oracle。
- 未来经济系统中，oracle 成本应转嫁给 seller；seller 上架 data 时可以选择 oracle 类型。Chainlink/CRE 更贵，但更能说服 buyer。

本迭代目标是先把设计想清楚并固定成文档，后续再实施合约、Worker、drop-script 改动。

## 目标

- 设计中心化 Cloudflare Worker oracle 的完整链路。
- 保持合约结构未来能兼容 CRE。
- 明确当前合约需要如何适配。
- 明确 `drop-script` 如何触发 Worker。
- 找出设计漏洞和必要防护。
- 形成可执行实施计划。

## 范围

本轮文档研究允许覆盖：

- Oracle 合约接口设计。
- Cloudflare Worker 触发流程。
- 用户传入 tx hash 的验证规则。
- Walrus availability 逻辑迁移。
- `drop-script` 集成点。
- 未来 `drop-sdk` 接口形态。
- 未来 seller 选择 oracle 类型的经济系统预留。

本轮不实施：

- 不写 Worker 代码。
- 已获用户确认后实施合约改造。
- 不改 `drop-script`。
- 已获用户确认后部署合约。
- 不部署 Cloudflare Worker。
- 合约部署已按用户确认发出 Arbitrum Sepolia 链上交易；不发 Worker report 交易。

实施状态：

- 已改造合约 Oracle 层为 hybrid centralized/CRE 兼容结构。
- 已让 `ExchangeHub` 管理员可更新 Oracle adapter、VSS verifier、VDD verifier。
- 已新增本地测试。
- 已通过本地合约测试。
- 已部署到 Arbitrum Sepolia。

## 结论概览

这个方向可行，但有几个硬要求：

1. Worker 不能信任用户传入的任何参数，只能信任链上交易 receipt/logs 和链上状态。
2. 用户传入 tx hash 后，Worker 必须验证该交易确实在 Arbitrum Sepolia 上成功执行。
3. Worker 必须验证 log 来自当前 TrustDrop OracleProxy 合约，且事件签名、requestId、callback、cCipher 都匹配预期。
4. Worker 写回链上时，合约必须校验 `msg.sender` 是授权 oracle signer/relayer，或者采用 CRE-compatible `onReport` 接口并校验 forwarder/signer。
5. 合约必须防重放：同一个 requestId 只能处理一次，且必须绑定 `cCipher`、callback、chainId、proxy 地址和 nonce。
6. 中心化 oracle 是可信执行者，不能给 buyer 提供和 Chainlink/CRE 同级别的可信度；产品上必须显式区分。
7. 如果未来要支持 seller 选择 oracle 类型，这个选择必须写入 sale/channel 状态，不能只由脚本约定。

## 推荐总体架构

### 合约层

保留 `IOracleProxy.request(bytes cCipher, address callback)` 作为 VDD 调用接口。

新增统一 receiver 风格的 OracleProxy：

```text
VDD._triggerOracle(cCipher)
  -> OracleProxy.request(cCipher, address(this))
       - require whitelist[msg.sender]
       - requestId = keccak256(chainid, address(this), callback, cCipher, nonce)
       - requests[requestId] = { cid: cCipher, client: callback, fulfilled: false }
       - emit OracleRequested(requestId, callback, cCipher, nonce, oracleMode)

Cloudflare Worker / CRE
  -> submit report to OracleProxy

OracleProxy
  -> verify sender / forwarder / signer / mode
  -> decode report
  -> validate request exists and not fulfilled
  -> response = abi.encode(status, endTime)
  -> callback.onResponse(cCipher, response)
  -> mark fulfilled / delete request
```

建议事件替换或扩展当前 `RequestSent`：

```solidity
event OracleRequested(
    bytes32 indexed requestId,
    address indexed client,
    bytes cCipher,
    uint256 nonce,
    uint8 oracleMode
);
```

建议 report payload：

```solidity
struct OracleReport {
    bytes32 requestId;
    bytes cCipher;
    uint256 status;
    uint256 endTime;
    bytes err;
}
```

链上提交接口建议拆成两个分支：

```solidity
function submitCentralizedReport(bytes calldata report, bytes calldata signature) external;
function onReport(bytes calldata metadata, bytes calldata report) external; // CRE-compatible
```

其中：

- `submitCentralizedReport` 给 Cloudflare Worker 用。
- `onReport` 给 CRE / KeystoneForwarder 用。
- 两者内部调用同一个 `_handleOracleReport(...)`。
- 这样合约状态机保持一致，未来从 Worker 切到 CRE 时不改 VDD/ExchangeChannel。

### Worker 层

Worker 接口建议：

```http
POST /oracle/fulfill
Content-Type: application/json

{
  "chainId": 421614,
  "txHash": "0x...",
  "requestLogIndex": 3
}
```

`requestLogIndex` 可选。如果用户不传，Worker 从 receipt 中筛选唯一的 Oracle request log；如果筛出多个，要求用户指定。

Worker 还需要提供状态页面：

```http
GET /status
```

`/status` 只显示系统是否可用，不显示具体余额和 secret：

```json
{
  "ok": true,
  "chainId": 421614,
  "oracleProxyConfigured": true,
  "relayerConfigured": true,
  "relayerBalanceSufficient": true,
  "relayerHasPendingTx": false,
  "walrusApiConfigured": true,
  "lastCheckedAt": "2026-06-23T00:00:00.000Z"
}
```

要求：

- Worker 使用一个新的专用私钥，后续由用户准备。
- 私钥只配置在 Cloudflare secret 中，不写入仓库、不写入文档、不回显。
- `/status` 可以显示 relayer 地址，也可以不显示；如果显示，只显示 address，不显示私钥或具体余额。
- `/status` 只显示 `relayerBalanceSufficient: true/false`，不显示具体 ETH 数量。
- 余额阈值作为配置，例如 `MIN_RELAYER_BALANCE_WEI`。
- `/status` 应检查 RPC 可用、合约地址格式、relayer 余额是否超过阈值、是否存在 pending tx、Blockberry API key 是否配置。

Worker 步骤：

1. 校验请求来源：
   - 只允许 POST。
   - 可要求 API token / HMAC / allowlisted frontend origin。
   - 但安全性不能依赖前端 origin，最终仍以链上验证为准。
2. 读取 Arbitrum Sepolia receipt：
   - `eth_getTransactionReceipt(txHash)`。
   - 确认 `status == 1`。
   - 确认 `chainId == 421614` 对应 RPC。
3. 解析 logs：
   - 找到 `address == ORACLE_PROXY_ADDRESS`。
   - event topic 是 `OracleRequested`。
   - 解码 `requestId`、`client`、`cCipher`、`nonce`、`oracleMode`。
4. 校验链上状态：
   - `OracleProxy.requests(requestId)` 存在。
   - request 的 `cid == cCipher`。
   - request 的 `client == client`。
   - request 未 fulfilled。
   - `oracleMode` 是 centralized，或 sale/channel 允许 centralized oracle。
5. 执行 Walrus availability 脚本：
   - `cCipher` bytes32 -> hex -> base64url Walrus BlobId。
   - HTTP GET `https://api.blockberry.one/walrus-mainnet/v1/blobs/{blobId}`。
   - 带 API key。
   - 读取 `endEpoch`。
   - 计算 status/endTime。
6. 构造 report：
   - `abi.encode(requestId, cCipher, status, endTime, err)`。
7. 写回链上：
   - Worker 用 oracle relayer 私钥发交易到 `OracleProxy.submitCentralizedReport(...)`。
   - 或 Worker 只签名 report，把交易留给用户/SDK 发。当前用户要求 Worker 触发链上交易，因此第一版由 Worker 发交易。
   - 发交易前必须检查 relayer 是否已有 pending tx；如果有，返回 `RELAYER_PENDING_TX`，不发新交易。
   - 发交易后等待 receipt 到达目标确认数，再返回成功。
8. 返回结果：
   - `requestId`
   - `status`
   - `endTime`
   - `txHash` of report transaction

### `drop-script` 集成

`drop-script` 当前流程在 `fulfill` 后等待 `oracleSuccessUntil(cCipher)`。

改造后建议：

1. Seller 调用 `fulfill`。
2. 解析 `fulfill` transaction receipt 中的 `OracleRequested` log。
3. 调用 Worker：

```http
POST WORKER_URL/oracle/fulfill
{
  "chainId": 421614,
  "txHash": "<fulfill tx hash>",
  "requestLogIndex": "<optional>"
}
```

4. Worker 返回 report tx hash。
5. `drop-script` 等待 report tx mined。
6. `drop-script` 继续轮询 `oracleSuccessUntil(cCipher)`。
7. 进入 settle。

未来 `drop-sdk` 可提供：

```ts
await trustDrop.oracle.fulfillFromTx({
  chainId: 421614,
  txHash: fulfillTxHash,
  mode: "centralized"
})
```

## 当前 Oracle 逻辑迁移

当前 `FunctionsConsumer_Walrus.sol` 内联 JS 逻辑要搬到 Worker：

- `initDate = 2025-12-16T00:00:00Z`
- `initEpoch = 20`
- `epochLength = 1209600`
- `hexToBase64Url`
- Blockberry Walrus mainnet URL：
  - `https://api.blockberry.one/walrus-mainnet/v1/blobs/${base64Url}`
- Header：
  - `accept: */*`
  - `x-api-key: <secret>`
- 结果：
  - HTTP/API 失败：`status=0,endTime=0`
  - `endEpoch > currentEpoch`：`status=2,endTime=calculatedEndTimestamp`
  - 否则：`status=1,endTime=calculatedEndTimestamp`

改进建议：

- Worker 使用 `Date.now()` 可以接受，因为中心化 Worker 不需要 DON consensus；但为了和 CRE 逻辑一致，建议封装 `nowSeconds()`，CRE 版本替换为 `runtime.now()`。
- 对 `endEpoch` 做类型和范围检查。
- 对 API timeout 做明确处理。
- 记录但不要返回 API key。

## 主要设计漏洞与修正建议

### 1. 用户传 tx hash 触发存在假请求风险

风险：

- 用户可能传入无关交易。
- 用户可能传入其他合约 emit 的相同 event。
- 用户可能传入失败交易。
- 用户可能传入旧请求重复触发。

修正：

- Worker 必须校验 receipt `status == 1`。
- Worker 必须校验 log address 是配置中的 `ORACLE_PROXY_ADDRESS`。
- Worker 必须校验 event topic。
- Worker 必须从链上读取 `requests[requestId]` 再校验 request 状态。
- Worker 必须处理已 fulfilled 请求，避免重复发交易。

### 2. 中心化 oracle 可作恶

风险：

- Worker 可以伪造 `status=2`。
- Worker 私钥泄露会导致任意 request 被 fulfill。
- Buyer 对中心化 oracle 的信任弱于 Chainlink/CRE。

修正：

- 产品和协议层显式标记 oracle mode。
- Seller 上架时选择 oracle mode，buyer 购买前可见。
- 合约只允许当前 sale 选择的 oracle mode 回写。
- 中心化 oracle 的 signer 地址公开，方便审计。
- 后续可支持多 signer / threshold signature，作为介于中心化与 CRE 之间的过渡方案。

### 3. Worker 免费但 gas 不免费

风险：

- Cloudflare Worker 免费不等于链上 report 交易免费。
- 如果 Worker 代发交易，oracle relayer 账户要承担 Arbitrum Sepolia ETH / 主网 gas。
- 未来生产环境成本需要经济设计。

修正：

- 第一版测试网由 Worker relayer 代发。
- 未来合约引入 oracle fee：
  - seller list 时选择 oracle mode 并锁定/支付 oracle fee。
  - centralized fee 低。
  - CRE fee 高。
  - settle/refund 时按执行结果结算。
- 如果 Worker 余额不足，应明确返回 `RELAYER_BALANCE_INSUFFICIENT`。

### 4. API key 和 Worker 访问控制

风险：

- Worker endpoint 被外部刷请求。
- Blockberry API key 被滥用。
- Worker 代发链上交易被 DoS 消耗 gas。

修正：

- Worker endpoint 加 API token。
- 只处理配置的 chainId / oracleProxy / event。
- 对 txHash 做去重缓存。
- 对 requestId 做 Durable Object / KV 防重入锁。
- 对 IP / account / txHash 限流。
- 只有链上 request 存在且未 fulfilled 才发交易。

### 5. Relayer nonce / pending transaction

风险：

- Cloudflare Worker 是无状态服务；多个请求同时进来时，可能使用同一个 nonce 发交易。
- 后发交易可能 replacement 前一笔交易。
- 某笔交易 pending 过久会导致后续 nonce 堵塞。
- Worker 进程无状态，不能只靠内存锁保证串行。
- 如果用户重复提交同一个 tx hash，可能导致 Worker 多次尝试发 report。

原型阶段约束：

- 不要求频率控制和负载均衡。
- 但必须保护 relayer 私钥的 nonce，不允许盲目并发发交易。

推荐第一版策略：严格单飞行交易。

每次发 report tx 前：

1. 查询 relayer address。
2. `pendingNonce = eth_getTransactionCount(relayer, "pending")`。
3. `latestNonce = eth_getTransactionCount(relayer, "latest")`。
4. 如果 `pendingNonce > latestNonce`，说明已有 pending tx：
   - Worker 不发新交易。
   - 返回 `RELAYER_PENDING_TX`。
   - 响应中可以提示稍后重试，但不显示 nonce 细节。
5. 如果 `pendingNonce == latestNonce`，再构造并发送交易。
6. 发送后等待 receipt：
   - 如果 mined 且成功，返回 report tx hash。
   - 如果 mined 但 reverted，返回 `REPORT_TX_REVERTED`。
   - 如果超时未 mined，返回 `REPORT_TX_PENDING`，并要求调用方稍后查状态或重试。

这个策略的优点：

- 无需 Durable Object / KV 队列。
- 无状态 Worker 可以安全运行。
- 不会产生多个同 nonce 交易互相顶替。
- 不会在前一笔 pending 时继续制造 nonce 队列。

缺点：

- 同一时间只能处理一笔 oracle report。
- 如果某笔交易长期 pending，Worker 会暂停处理新请求。

后续更合理的生产策略：

- 使用 Cloudflare Durable Object 为 relayer 地址提供单线程 nonce manager。
- 将 requestId 加入 KV/D1，记录状态：`queued -> submitted -> mined -> failed`。
- 支持查询 `/oracle/status/:requestId`。
- 对 pending 太久的交易做显式 speed-up 或 cancel，但必须有清晰的 nonce 管理和 gas bump 策略。

本原型阶段不建议自动 speed-up/cancel：

- 自动 replacement 逻辑容易造成 nonce 死锁或意外顶替。
- 如果出现 pending 卡死，先人工处理 relayer 账户 nonce。

合约侧额外防护：

- `submitCentralizedReport` 必须检查 request 未 fulfilled。
- 如果 Worker 因网络错误重试同一 report，第二次链上应 revert 或无效，不应重复写状态。

### 6. reorg / finality

风险：

- 用户刚提交交易就触发 Worker，receipt 还可能被 reorg。

修正：

- 测试网第一版可接受 `N=1` confirmation。
- 稳定版配置 `MIN_CONFIRMATIONS`。
- Worker 如果 confirmations 不够，返回 `PENDING_CONFIRMATIONS`，drop-script 延迟重试。

### 7. cCipher 编码仍是核心风险

风险：

- 当前历史问题是 Solidity `bytes` 到 Walrus BlobId string 编码不清晰。
- Worker 解析 event 后必须严格按 bytes32 -> base64url 转换。

修正：

- 写独立测试：
  - 给定 `bytes32 cCipher`。
  - 期望 base64url blob id。
  - 与当前 Walrus/Blockberry API 路径一致。
- 合约 event 不应 emit string，继续 emit raw bytes 更可靠。

### 8. 两个 oracle 分支的状态一致性

风险：

- centralized 和 CRE 两条路径都能写 `oracleSuccessUntil`，如果没有 mode 约束会互相覆盖。

修正：

- sale/listing 层记录 `oracleMode`。
- request 记录 `oracleMode`。
- report 必须匹配 request 的 oracleMode。
- 已 fulfilled 后不能被另一个 mode 覆盖。

## 合约适配建议

### 短期第一版

新增或替换 `OracleProxy` 为兼容双模式：

```solidity
enum OracleMode {
    Centralized,
    ChainlinkCRE
}

struct RequestContext {
    bytes cid;
    address client;
    OracleMode mode;
    bool fulfilled;
}
```

最小接口：

```solidity
function request(bytes memory cCipher, address callback) external;
function submitCentralizedReport(bytes calldata report) external;
function onReport(bytes calldata metadata, bytes calldata report) external;
function setOracleSigner(address signer, bool allowed) external onlyOwner;
```

第一版 `submitCentralizedReport` 可以只允许 `msg.sender == centralizedOracleSigner`。

更稳妥版本：

- report 内包含 signer 签名。
- 任何人可提交 report。
- 合约验证 EIP-712 signature。
- 这样 Worker 可以只签名，不一定亲自付 gas。

但用户当前要求 Worker 触发链上交易，所以第一版可以先让 Worker 作为 relayer 直接发交易。后续再升级为 EIP-712。

### 中期支持 seller 选择 oracle

需要在 sale/listing 数据中加入 oracle mode：

- `listFile(..., oracleMode, oracleFeePolicy)`。
- `PurchaseEvent` / subgraph schema 增加 oracle mode。
- `ExchangeInfo` 或 sale version 中绑定 oracle mode。
- `fulfill` 调用 Oracle 时使用该 sale 的 oracle mode。

短期如果不改 sale 数据结构，也可以全局配置当前系统只用 centralized oracle。但这不能满足未来 seller 可选 oracle 的目标。

### CRE 兼容要求

即使当前不用 CRE，合约也应预留：

- `onReport(bytes metadata, bytes report)`。
- forwarder 地址配置。
- workflow allowlist 配置。
- 内部 `_handleOracleReport` 与 centralized path 共用。

## Cloudflare Worker 实施计划

目录建议：

```text
oracle-worker/
  package.json
  wrangler.toml
  src/index.ts
  src/chain.ts
  src/walrus.ts
  src/abi.ts
  src/report.ts
  test/
```

环境变量 / secrets：

- `ARBITRUM_SEPOLIA_RPC_URL`
- `ORACLE_PROXY_ADDRESS`
- `ORACLE_RELAYER_PRIVATE_KEY`
- `MIN_RELAYER_BALANCE_WEI`
- `BLOCKBERRY_API_KEY`
- `WORKER_API_TOKEN`
- `MIN_CONFIRMATIONS`

Cloudflare secrets：

```sh
wrangler secret put ARBITRUM_SEPOLIA_RPC_URL
wrangler secret put ORACLE_RELAYER_PRIVATE_KEY
wrangler secret put BLOCKBERRY_API_KEY
wrangler secret put WORKER_API_TOKEN
```

Worker endpoint：

- `POST /oracle/fulfill`
- `GET /health`: 轻量 liveness，不查链。
- `GET /status`: 查配置、RPC、relayer 余额是否充足、是否存在 pending tx；不显示具体余额。

错误码建议：

- `UNAUTHORIZED`
- `UNSUPPORTED_CHAIN`
- `TX_NOT_FOUND`
- `TX_FAILED`
- `REQUEST_LOG_NOT_FOUND`
- `MULTIPLE_REQUEST_LOGS`
- `REQUEST_NOT_FOUND_ONCHAIN`
- `REQUEST_ALREADY_FULFILLED`
- `WALRUS_API_FAILED`
- `RELAYER_BALANCE_INSUFFICIENT`
- `RELAYER_PENDING_TX`
- `REPORT_TX_FAILED`
- `REPORT_TX_REVERTED`
- `REPORT_TX_PENDING`
- `PENDING_CONFIRMATIONS`

Relayer 交易策略：

- 第一版严格要求 `pendingNonce == latestNonce` 才发送交易。
- 第一版发送后等待 receipt，不做后台队列。
- 如果已有 pending tx，直接返回 `RELAYER_PENDING_TX`，由 `drop-script` 延迟重试。
- 不自动 speed-up/cancel pending tx。
- 不在响应中显示具体 nonce、余额或 gas 细节，避免暴露 relayer 状态。

## drop-script 改造计划

新增配置：

- `ORACLE_MODE=centralized`
- `ORACLE_WORKER_URL`
- `ORACLE_WORKER_TOKEN`
- `ORACLE_WORKER_STATUS_URL` 可选，默认 `${ORACLE_WORKER_URL}/status`

流程改造：

1. `stage_3_fulfill` 发出 fulfill 交易。
2. 保存 fulfill tx hash。
3. 如果 `ORACLE_MODE=centralized`：
   - 可先调用 `/status`，确认 `relayerBalanceSufficient=true` 且 `relayerHasPendingTx=false`。
   - 调用 Worker `/oracle/fulfill`。
   - 传 `chainId` 和 `txHash`。
   - 等待 Worker 返回 report tx hash。
   - 等待 report tx mined。
4. 继续 `wait_for_oracle_signal`。

边界：

- 如果 Worker 返回 `PENDING_CONFIRMATIONS`，脚本延迟重试。
- 如果 Worker 返回 `RELAYER_PENDING_TX`，脚本延迟重试，不应再次触发新的链上请求。
- 如果 Worker 返回 `REQUEST_ALREADY_FULFILLED`，脚本直接进入轮询。
- 如果 Worker 返回 fatal error，停止，不盲目重试。

未来 SDK：

```ts
await sdk.oracle.fulfillFromTx({
  txHash,
  mode: "centralized",
  workerUrl,
  token
})
```

## 经济系统预留

未来 seller 上架时选择：

```text
oracleMode = centralized | chainlinkCRE
oracleFee = mode-specific estimated cost
```

设计方向：

- centralized：低成本、低可信。
- Chainlink/CRE：高成本、更能 convince buyer。
- buyer 在 purchase 前看到 oracle mode。
- seller 支付或预存 oracle fee。
- 如果 oracle 失败，费用如何退还需要单独设计。
- 如果 buyer 触发 centralized Worker，不能由 buyer 长期承担 seller 的可用性证明成本；最终成本应绑定到 seller/listing。

短期先不实现经济系统，但合约事件和数据结构设计时不要堵死这个方向。

## 实施阶段

### 阶段 1: 合约设计定稿

- 确认是否第一版全局只用 centralized oracle。
- 确认是否立刻把 `oracleMode` 写入 sale 数据。
- 确认 report 接口采用 `msg.sender == signer` 还是 EIP-712 signature。

建议：

- 第一版为了尽快跑通：全局 centralized oracle + `msg.sender == centralizedOracleSigner`。
- 同时预留 `onReport` 和 mode 字段结构。
- 第二版再做 seller-level oracle mode。

已实施第一版：

- `OracleProxy` 保留 `request(bytes,address)` 和 `setWhitelist(address,bool)`。
- `OracleProxy` 新增：
  - `OracleMode`
  - `centralizedOracleSigner`
  - `creForwarder`
  - `defaultMode`
  - `submitCentralizedReport(bytes)`
  - `onReport(bytes,bytes)`
  - 统一 `_handleOracleReport`
- `submitCentralizedReport` 当前采用 `msg.sender == centralizedOracleSigner`。
- `onReport` 当前采用 `msg.sender == creForwarder`。
- Oracle report 回调必须成功，否则整个 report 交易回滚，request 不会被标记为 fulfilled，避免一次坏回调永久锁死请求。
- `centralizedOracleSigner` 部署时可为 `address(0)`，等待 Worker 专用私钥准备后由 owner 配置。
- `ExchangeHub` 将 `oracleWrapper`、`vssVerifier`、`vddVerifier` 改为 owner 可配置。
- `ExchangeHub.createExchangeChannel` 会自动把新 channel 加入 OracleProxy whitelist。
- VSS/VDD verifier 合约未重新部署，继续沿用当前地址。

### 阶段 2: 合约实现

- 新增 `OracleProxyHybrid.sol` 或替换 `OracleProxy.sol`。
- 新增 centralized report path。
- 新增 CRE-compatible `onReport` path。
- 更新 deploy script。
- 更新测试。

验收：

- [x] `forge build --root contracts`
- [x] `forge test --root contracts`
- [x] request/report/replay/unauthorized tests 通过。

测试结果：

- `forge test --root contracts`：26 passed, 0 failed。
- 新增 `contracts/test/OracleProxyHybrid.t.sol`，覆盖：
  - centralized report 正常回调。
  - 非 signer 不能 submit centralized report。
  - 同一 request 不能 replay。
  - CID mismatch 拒绝。
  - report 回调失败时回滚，且 request 不会被标记为 fulfilled。
  - CRE forwarder `onReport` 正常回调。
  - 非 forwarder 不能 `onReport`。
  - Hub owner 可更新 oracle/verifier，且新 channel 使用新配置并自动 whitelist。

部署结果：

| 组件 | 地址 | 说明 |
| --- | --- | --- |
| OracleProxy | `0x13A59912Fe91211FB7a901974997F716f11EcFe8` | hybrid centralized/CRE oracle |
| ExchangeHub | `0x1C01E8E981909926Ed67B5eEfAbfDfeCAcC882a1` | 最新 Hub |
| ExchangeChannelImplementation | `0xAf34AE4156d304f8C65F5Fa211A9005B0477bbd6` | 最新 channel logic |
| VSS verifier | `0x5e80ed679fb9f4050a5c7ede5ccbe39178f142a2` | 沿用旧 verifier |
| VDD verifier | `0x154D59Ed30B7784B5c9324b32b9ec5d6c8DE4071` | 沿用旧 verifier |

链上校验：

- `ExchangeHub.oracleWrapper()` -> `0x13A59912Fe91211FB7a901974997F716f11EcFe8`
- `ExchangeHub.vssVerifier()` -> `0x5e80ed679fb9f4050a5c7ede5ccbe39178f142a2`
- `ExchangeHub.vddVerifier()` -> `0x154D59Ed30B7784B5c9324b32b9ec5d6c8DE4071`
- `OracleProxy.controller()` -> `0x1C01E8E981909926Ed67B5eEfAbfDfeCAcC882a1`
- `OracleProxy.creForwarder()` -> `0x76c9cf548b4179F8901cda1f8623568b58215E62`
- `OracleProxy.centralizedOracleSigner()` -> `0x5318831f07e8E5e3e8Fdf2a53ef0F0c3996a88dF`
- `OracleProxy.defaultMode()` -> `0` centralized

本轮部署后 checklist 结果：

- `drop-script/scripts/check-env.sh --section contracts`：21 PASS，0 ACTION_REQUIRED。
- `drop-script/scripts/check-env.sh --section oracle`：7 PASS，1 WARN，2 ACTION_REQUIRED，1 INFO。剩余操作项是 Worker 本体尚未部署/配置，以及 Worker signer gas 余额仍需按运行期策略确认。
- `pnpm --dir subgraph codegen` 已通过。
- `pnpm --dir subgraph build` 已通过。

已完成 signer 配置：

- Worker 专用私钥已加入 `contracts/.env`：
  - `ORACLE_RELAYER_PRIVATE_KEY=<worker signer private key>`
  - `ORACLE_PROXY_ADDRESS=0x13A59912Fe91211FB7a901974997F716f11EcFe8`
- oracle signer 地址已确认：`0x5318831f07e8E5e3e8Fdf2a53ef0F0c3996a88dF`。
- 已从 `0xB34Cdac031d3bF18e014f8e9ce17DDA9cdb9EbE9` 向 signer 转入 `0.05 ETH`。
  - funding tx: `0x27ac6a4408eedc521854c3871050d6794c71cd99f9c913656a1b13d16b51e6c1`
- 已用部署管理员私钥调用配置脚本：

```sh
cd contracts
set -a
source .env
set +a
forge script script/SetCentralizedOracleSigner.s.sol:SetCentralizedOracleSigner \
  --rpc-url https://sepolia-rollup.arbitrum.io/rpc \
  --broadcast
```

- 该脚本从 `ORACLE_RELAYER_PRIVATE_KEY` 推导 signer 地址，并调用 `OracleProxy.setCentralizedOracleSigner(<worker signer address>)`；脚本只打印地址，不打印私钥。
- signer config tx: `0xc49b7f1398c95e52b14c34285452c8142255ef0576ed0aaed94c6f6c8cb50dd8`

后续必须配置：

- 部署 Worker 前，centralized report path 不能实际回写。
- `drop-script/.env` 的 `HUB_ADDRESS` 已更新到最新 Hub。
- `subgraph/subgraph.yaml` 已更新到最新 Hub 和 startBlock，`pnpm --dir subgraph codegen && pnpm --dir subgraph build` 已通过；尚未部署 subgraph。

### 阶段 3: Worker 实现

- 已新建 `oracle-worker/` 项目。
- 已实现 `GET /health`、`GET /status`、`POST /oracle/fulfill`。
- 已实现 API token 认证。
- 已实现 receipt/log 解析：
  - 校验 Arbitrum Sepolia chain id。
  - 校验 receipt `status == success`。
  - 校验 receipt 至少达到 `MIN_CONFIRMATIONS`，否则返回 `PENDING_CONFIRMATIONS`。
  - 只接受来自当前 `ORACLE_PROXY_ADDRESS` 的 `OracleRequested` log。
  - 如果同一 receipt 有多个 request log，要求用户传 `requestLogIndex`。
- 已实现链上 request 状态复核：
  - `requests(requestId)` 存在。
  - `client` 与 log 一致。
  - `cid` 与 log 一致。
  - `mode == Centralized`。
  - 已 fulfilled 时直接返回 `alreadyFulfilled`。
- 已迁移 Walrus availability 逻辑：
  - `cCipher` hex bytes -> base64url Walrus blob id。
  - 请求 Blockberry Walrus mainnet blob API。
  - 按旧 Chainlink Functions 逻辑计算 `status` 和 `endTime`。
- 已实现 relayer 防 nonce 冲突：
  - 发交易前检查 `latestNonce == pendingNonce`。
  - 如果 relayer 有 pending tx，返回 `RELAYER_PENDING_TX`，不发新交易。
- 已实现链上 report：
  - 构造 `abi.encode(requestId, cCipher, status, endTime, err)`。
  - 用 `ORACLE_RELAYER_PRIVATE_KEY` 调用 `OracleProxy.submitCentralizedReport(bytes)`。
  - 等待 report tx receipt。

验收：

- [x] `pnpm --dir oracle-worker install` 完成。
- [x] `pnpm --dir oracle-worker build` 通过。
- [x] `pnpm --dir oracle-worker exec wrangler deploy --dry-run` 通过，Worker 打包大小约 `696.34 KiB`，gzip 约 `140.13 KiB`。
- [ ] Worker 单元测试尚未补。
- [x] Worker 已部署到 Cloudflare。
  - URL: `https://trustdrop-oracle-worker.zhengxingao.workers.dev`
  - Initial Version ID: `41b834a0-d687-4041-b1d4-e37ee0dbd997`
  - Current Version ID: `397d1292-c515-4edb-8bdb-edb688715830`
- [x] Worker `/health` 返回 `ok=true`。
- [x] Worker `/status` 返回 `ok=true`，并确认：
  - chain id 是 `421614`。
  - relayer 已配置。
  - relayer 与 `OracleProxy.centralizedOracleSigner()` 匹配。
  - relayer balance sufficient。
  - relayer 无 pending tx。
  - Walrus API key 已配置。
- [x] 已用真实 `OracleRequested` tx 验证 Worker report。
  - IntegrationClient: `0x4A0818E005a7f0D6F3B9182142cA654749F41d19`
  - request tx: `0x4050d607ba421c7062f236c8413becc9286825203ffe28d88ea307f5805878ef`
  - requestId: `0x53e640a7f038d15a1d3eb9a0c9c4a8e8fc6a5e89bffe1458e85971fafc74462a`
  - cCipher: `0x4c605762bd249b798bbf2347b7a6d05db2c7b25051e4703057c98043e1c5248a`
  - report tx: `0x7671732c6932848c760166827ae6616fdb512b09a81f96965ab848b140fd1bb9`
  - callback result: `lastStatus=1`, `lastEndTime=1770681600`
  - 结论：Worker 能解析真实 receipt/log，能查 Walrus mainnet Blockberry API，能调用 `OracleProxy.submitCentralizedReport(bytes)`，并能触发 client callback。

部署记录：

- `pnpm --dir oracle-worker exec wrangler whoami` 已确认登录账号 `zhengxingao@live.cn`，具备 Workers write 权限。
- Wrangler secrets 已上传：
  - `ARBITRUM_SEPOLIA_RPC_URL`
  - `ORACLE_RELAYER_PRIVATE_KEY`
  - `BLOCKBERRY_API_KEY`
  - `WORKER_API_TOKEN`
- `pnpm --dir oracle-worker run deploy` 已成功部署 Worker。
- Worker status 脚本已固化为 `drop-script/scripts/check-oracle-worker-status.sh`。
- Worker fulfill 脚本已固化为 `drop-script/scripts/fulfill-oracle-worker-from-tx.sh <tx-hash> [request-log-index]`。
  - 脚本从 `drop-script/.env` 读取 `ORACLE_WORKER_TOKEN`，不打印 token。
  - 用已 fulfilled 的 request 复测返回 `alreadyFulfilled=true`，未重复发 report 交易。
- Blockberry/Walrus API key 已用已知 Walrus mainnet blob 测试，返回 HTTP 200，响应包含 `blobIdBase64`、`startEpoch`、`endEpoch`、`size`。

### 阶段 4: drop-script 集成

- 已在 `drop-script` 中加入 opt-in Worker 触发：
  - 默认 `ORACLE_MODE=external` 或未设置时，不触发 Worker，保持旧行为。
  - 设置 `ORACLE_MODE=centralized` 后，必须提供 `ORACLE_WORKER_URL` 和 `ORACLE_WORKER_TOKEN`。
  - fulfill 交易成功后调用 Worker `/status`。
  - `/status.ok == true` 后调用 `/oracle/fulfill`，传 `chainId` 和 fulfill tx hash。
  - Worker 返回 `reportTxHash` 后继续轮询 `oracleSuccessUntil`。

验收：

- [x] `PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p drop-script` 通过。
- [x] `drop-script/.env` 已启用 `ORACLE_MODE=centralized`。
- [x] `drop-script/.env` 已配置 `ORACLE_WORKER_URL`、`ORACLE_WORKER_TOKEN`、`ORACLE_WORKER_STATUS_URL`。
- [x] 在 Arbitrum Sepolia 上最小 request 后 Worker 能回写。
- [x] 本地合约测试已验证 `OracleProxy.submitCentralizedReport` 能回调真实 Channel 并更新 `oracleSuccessUntil`。
- [x] 本地合约测试已验证 `oracleSuccessUntil` 满足窗口后 `settle` 能继续执行。
- [ ] 完整 `drop-script` live flow 尚未跑完；该流程会触发 SP1 Prove Network 证明和 Walrus 读写，不能作为普通编译/轻量检查默认执行。

### 阶段 5: 文档和 checklist

- 更新 0005 checklist：
  - Cloudflare Worker URL。
  - Worker health。
  - oracle signer 地址。
  - Worker relayer ETH balance。
  - centralized/CRE mode。
- 更新 `.codex/docs/drop-script.md`。
- 更新 `.codex/docs/contracts.md`。

当前 checklist 结果：

- `drop-script/scripts/check-env.sh --section env`：22 PASS，2 WARN，0 ACTION_REQUIRED。
  - WARN 是可选 `CENTRALIZED_ORACLE_SIGNER` / `CRE_FORWARDER` 未显式配置。
- `drop-script/scripts/check-env.sh --section oracle`：10 PASS，1 WARN，0 ACTION_REQUIRED，1 INFO。
  - WARN 是 `cCipher` bytes-to-Walrus-id 编码仍需在真实全流程中验证。

## 当前开放问题

- 第一版是否需要 seller-level oracle mode，还是先全局 centralized。
- Worker report 是由 Worker 直接发交易，还是签名后由用户/drop-script 发交易。
- 是否要用 EIP-712 report signature，还是第一版只校验 `msg.sender`。
- Worker 第一版已按严格单飞行交易保持无状态；如果后续要并发处理，再引入 Durable Object 做 nonce manager。
- Blockberry API 是否稳定，是否需要备用 Walrus API。

## 测试验收标准

设计阶段：

- [x] 识别用户传 tx hash 的信任边界。
- [x] 识别中心化 oracle 的安全弱点。
- [x] 给出合约双分支兼容方案。
- [x] 给出 Worker 实施计划。
- [x] 给出 drop-script 集成计划。
- [x] 给出经济系统预留方向。

后续实现阶段：

- [x] 合约 build/test 通过。
- [x] Worker TypeScript build 通过。
- [ ] Worker 单元测试通过。
- [x] Worker 能解析真实 fulfill/request tx receipt。
- [x] Worker 能写回 OracleProxy。
- [x] Worker `/status` 能返回 relayer 余额是否充足，但不显示具体余额。
- [x] Worker 在 relayer 有 pending tx 时返回 `RELAYER_PENDING_TX`，不发新交易。
- [x] `drop-script` 已加入 Worker 触发逻辑并通过编译。
- [x] Worker 已部署，`drop-script` 已配置 centralized mode。
- [x] 本地合约层已覆盖：Channel fulfill -> OracleProxy request -> centralized report -> Channel `oracleSuccessUntil` 更新 -> settle。
- [ ] 完整 `drop-script` live flow 通过。

当前剩余工作：

1. 如需继续 live full-flow，必须显式批准后再跑完整 `drop-script`，因为它会触发 SP1 Prove Network 证明和 Walrus 网络读写。
2. live full-flow 的首要观察点是：完整 Channel fulfill tx 是否 emit `OracleRequested`，Worker 是否返回 `reportTxHash`，对应 Channel 的 `oracleSuccessUntil(cCipher)` 是否更新。
3. 如完整流程失败，优先区分是 drop-script 参数/证明/通道状态问题，还是 Worker/OracleProxy 问题；当前最小链上集成测试与本地合约测试已经证明 Worker/OracleProxy/Channel callback 基础链路可用。

最新本地验收：

- `forge test --root contracts --match-contract ExchangeTest`：2 passed。
- `forge test --root contracts`：27 passed。
- 新增 `test_OracleProxyReportUpdatesChannelAndAllowsSettle`：
  - 不使用直接 `vm.prank(address(oracleProxy))` 伪造回调。
  - 通过 `channel.fulfill` 触发真实 `OracleProxy.request`。
  - 通过 `OracleProxy.submitCentralizedReport` 进入真实 callback。
  - 断言 `oracleSuccessUntil(cCipher)` 更新。
  - 断言 `settle` 成功。

## 经验总结

- 中心化 Worker 可以作为 CRE 迁移前的工程过渡，但必须在协议和产品层明确可信度差异。
- 合约应抽象成“请求 + 报告”模型，而不是绑定某个 oracle vendor。
- 用户主动触发 Worker 是可行的，但 Worker 必须只相信链上 receipt/logs/state。
- 未来 CRE 兼容性应体现在 `onReport` 和统一 `_handleOracleReport`，避免后续二次大改 VDD/ExchangeChannel。
- 手写带 token 的 curl 容易出现 JSON 引号错误；以后通过 `drop-script/scripts/fulfill-oracle-worker-from-tx.sh` 触发 Worker。
- 完整 `cargo run -p drop-script` 不是轻量检查；它会进入 SP1 Prove Network/Walrus 业务链路。开发阶段应优先用合约测试、Worker 最小集成脚本和环境 checklist 分段验证。
