# 迭代: Drop Script 完整流程逻辑审查

## 背景

上一轮已经完成 `drop-script`、合约、subgraph、VSS/VDD guest 与 script 的结构性准备，并完成了合约和 subgraph 的部署。当前进入迭代 0002，本轮不跑代码，不改实现，目标是先从协议设计和静态代码两个层面审查 `drop-script` 的完整流程是否逻辑闭合。

用户明确要求本轮先从以下 8 个角度研究：

1. 参与方与信任边界
2. 资产与承诺绑定
3. 购买与密钥释放流程
4. VSS 证明逻辑集成
5. VDD / Walrus RSLH-VE 证明逻辑集成
6. Oracle 与可用性判断
7. 合约状态机与经济安全
8. `drop-script` 实现与协议意图一致性

## 目标

- 论证当前设计在单 seller、单 channel、单全局数据密钥的开发假设下是否有效。
- 识别当前设计距离生产安全还差哪些协议约束。
- 静态检查 `drop-script`、合约、VSS/VDD guest、`drop-lib`、Oracle、subgraph 的集成逻辑。
- 形成可以指导后续代码修复和端到端调试的报告。

## 范围

本轮初始阶段只允许修改本迭代文档。

用户已批准实施以下范围：

- 合约 `purchase` 强制 `dataCommitment` 与 `dataVersion` 绑定。
- 合约 `purchase` 增加 deadline 合理性检查。
- `drop-script` 精确解析 `PurchaseEvent`。
- `drop-script` 精确解析 `ExchangeChannelCreated`。

第 5 项 Oracle / Walrus `cCipher` 编码链路，本轮先深入论证方案，不直接实施。

本轮不处理：

- 不运行 `drop-script`、guest execute、证明或部署命令。
- 不修改 subgraph、配置或依赖。
- 不研究 VSS/VDD 的密码学证明细节到形式化证明层面。
- 不泄露 `.env`、私钥、API key、deploy key。

## 实施方法

审查方式：

- 以 `drop-script/src/main.rs` 为主线，按 stage 追踪链上调用、证明生成、事件解析、恢复和结算。
- 对照 `contracts/src/ExchangeChannel.sol`、`VSS.sol`、`VDD.sol`、`OracleProxy.sol`、`FunctionsConsumer_Walrus.sol` 检查链上约束。
- 对照 `guest/vss/program/src/main.rs`、`guest/vdd/program-vdd-walrus-rslhve/src/main.rs`、`drop-lib/src/rslh_ve.rs` 检查 public values 和 binding hash 是否匹配。
- 对照 subgraph mapping 检查事件索引是否足够支撑后续调试。
- 将问题分为设计问题、实现问题、工程限制和后续验证项。

## 研究笔记

### 总体结论

当前方案在开发环境的窄假设下是逻辑可闭合的：

- seller 先发布 data commitment 和 encrypted asset。
- buyer purchase 时把 sale、price、deadline、data commitment、VSS key commitment 绑定进 purchase digest。
- seller fulfill 时提交 VSS 证明和 VDD 证明，并触发 Oracle。
- buyer 可以从 VSS 释放的 encrypted data key 恢复数据密钥。
- seller 只能在 VSS privy、VDD verified、Oracle availability 三个条件同时满足后 settle。
- buyer 可以在 deadline 后 refund。

但是当前方案还不能直接视为生产安全协议。核心原因是：VDD 是抽样证明而不是完整确定性证明，合约缺少若干 marketplace 级别的强约束，`drop-script` 的事件解析与密钥恢复仍偏单次演示流程，Oracle 对 Walrus blob id 的编码链路需要进一步验证。

本轮结论：

- 对单次开发演示：设计有效，可以进入下一阶段代码修复和端到端调试。
- 对真实资产交易：需要先修复下文 P1/P2 问题，并补充 VDD 抽样安全参数论证。

### 1. 参与方与信任边界

参与方：

- seller：部署或拥有 channel，发布 sale，持有原始数据和数据密钥，负责生成 VSS/VDD 证明。
- buyer：发起 purchase，支付 ETH，提供 VSS key commitment，后续恢复数据密钥。
- ExchangeHub / ExchangeChannel：托管资金、记录 sale 和 purchase 状态、执行 settle/refund。
- VSS：验证 seller 对数据密钥释放的证明，并记录 buyer 是否已成为 privy。
- VDD：验证密文、原文承诺和数据密钥承诺之间的关系，并触发 Oracle。
- OracleProxy / WalrusFunctionsConsumer：检查 Walrus blob availability。
- SP1 Prover Network：生成 VSS/VDD 证明。
- subgraph：索引事件，辅助观察和调试，不应作为安全来源。

有效性判断：

- 合约是最终资金状态来源。
- SP1 verifier 是 VSS/VDD 证明有效性的来源。
- Oracle 只提供可用性信号，不证明密文正确性。
- subgraph 只做索引，不能参与安全判断。

主要风险：

- 当前 channel 设计更像“单 seller、单全局数据密钥”的交易通道。如果希望一个 channel 支持多资产、多密钥、多 buyer 并发，需要扩大状态粒度。
- seller 负责生成证明，buyer 不能强制 seller fulfill，只能过期 refund。这是协议设计选择，不是代码 bug。

### 2. 资产与承诺绑定

设计有效点：

- sale id 使用 `keccak256(abi.encodePacked(channel, chainid, nonce))`，可以隔离不同链、不同 channel、不同 nonce。
- `purchase` 的 exchange digest 绑定 buyer、sale digest、data version、price、init time、deadline、data commitment、VSS key commitment。
- VSS binding hash 和合约一致：`abi.encode(dataKeyCommitment, c_keys, encryptedDataKeys)`。
- VDD binding hash 和合约一致：`abi.encode(cOrigin, dataKeyCommitment, cCipher)`。

关键问题：

- `purchase` 接收 buyer 传入的 `dataCommitment`，但合约没有强制 `keccak256(dataCommitment) == dataVersion`。这意味着 buyer 可以用当前 sale 的 `dataVersion`，但提供另一个 `dataCommitment`。seller 可以拒绝 fulfill，资金最终 refund，因此主要是 griefing 和状态一致性风险；但如果后续脚本或 UI 没有严格检查，会造成错误流程。
- 合约 `listFile` 事件包含 price，但 channel 没有存储并强制 purchase price 等于 listing price。当前语义更接近 buyer offer，listing price 只是链下提示。如果目标是固定标价销售，应在合约中存储 price 并在 purchase 中检查。

建议：

- P1：`purchase` 增加 `require(keccak256(dataCommitment) == dataVersion)`。
- P2：明确“buyer 出价”还是“listing 固定价”。如果是固定价，合约必须存储 sale price 并校验。

### 3. 购买与密钥释放流程

设计有效点：

- buyer purchase 后资金进入 channel 锁定余额。
- seller fulfill 时必须在 deadline 前执行。
- buyer 通过 VSS 释放的 encrypted data key 恢复数据密钥。
- settle 和 refund 都会删除 pending exchange，避免同一 purchase 被重复结算或退款。

关键问题：

- `purchase` 没有 deadline 下限和上限。buyer 可以设置已过期或非常近的 deadline，导致 seller 无法 fulfill，只能 buyer 立即 refund；也可以设置极长 deadline，造成长期挂起状态。
- `dataKeyCommitment` 在 VSS 中是 channel 全局且只能提交一次。若 channel 只卖同一个全局数据密钥加密的数据，这是合理设计；若 channel 支持多个资产或多版本密钥，这是明显限制。
- `isPrivy(buyer)` 是 channel 全局状态。buyer 一旦成为 privy，后续 fulfill 可跳过 VSS。这个设计只在“channel 全部资产共享同一数据密钥”时安全。
- ECIES ephemeral public key 目前只存在于 `drop-script` 的内存状态中。若 purchase 和 recovery 跨进程运行，seller 无法仅凭链上事件恢复 VSS encrypted key 的解密材料。

建议：

- P1：增加 deadline 合理区间检查，例如 `deadline > block.timestamp + minWindow` 且 `deadline < block.timestamp + maxWindow`。
- P1：明确 channel 的密钥模型。如果坚持单全局数据密钥，需要在文档、UI 和脚本中禁止多密钥语义；如果要支持多资产独立密钥，应把 data key commitment 和 privy 状态改成 sale/data 粒度。
- P2：将 ECIES ephemeral public key 持久化到 purchase 参数、事件或链下订单元数据。

### 4. VSS 证明逻辑集成

设计有效点：

- `submitDataKeyCommitment` 先写入 channel 全局 data key commitment。
- `shareDataKey` 校验 SP1 verifier、public values 和 binding hash。
- `drop-script` 生成 VSS proof 后提交 `vssKeyCommitment` 和 `encryptedDataKey`。
- `shareDataKey` 成功后会记录 buyer 的 privy 状态，settle 依赖该状态。

实现风险：

- `stage_4_recovery` 通过查询 channel 上所有 `DataKeyShared` 事件再取最后一个匹配 buyer 的事件来恢复密钥。单次演示可用，但并发 purchase、多 buyer、多次 fulfill 时不稳。
- `fulfill` 中如果 buyer 已经是 privy，则不会再次调用 VSS，也不会为本次 purchase 产生新的 `DataKeyShared` 事件。该逻辑与 channel 全局数据密钥一致，但与“每次 purchase 都独立释放密钥”的直觉不一致。
- 脚本会输出 asset key、secret sharing key 等敏感调试信息。开发环境可以接受，任何共享日志或 CI 都不应保留。

建议：

- P2：`stage_4_recovery` 应从 fulfill receipt 或指定 block/tx 过滤事件，而不是取历史最后一条。
- P2：明确 repeat buyer 的恢复策略。如果 buyer 已经 privy，应跳过恢复或复用本地已持有的数据密钥。
- P3：去掉默认敏感日志，改为显式 debug 开关。

### 5. VDD / Walrus RSLH-VE 证明逻辑集成

设计有效点：

- VDD guest 读取 `c_origin`、`c_cipher`、`c_key`、`aux_data` 和 `key`。
- guest 检查 `c_key == blake3(key)`。
- guest 调用 RSLH-VE 验证逻辑，并提交 public values `[c_origin, c_key, c_cipher]`。
- 合约 `VDD.submitVDDProof` 验证 proof 后记录 `vddVerified[cCipher] = true`，并触发 Oracle。
- VDD binding hash 与 host/script 的计算方式一致。

关键安全点：

- 当前 RSLH-VE 是抽样验证。`drop-lib` 默认 sample count 为 15，总 shard 数在 guest 调用中为 1000。它能提供概率性检测能力，但不是完整确定性证明。
- 安全性取决于抽样随机种子、样本数量、攻击者可污染 shard 比例、Walrus 编码结构和 RSLH-VE 证明构造。如果没有形式化参数说明，不能声称“完整文件一定正确加密”。
- 抽样种子由 `sha256(c_origin || c_cipher || c_key)` 派生，能把样本选择绑定到声明对象，避免 prover 自由选择样本。但仍需要证明 prover 无法通过构造承诺影响抽样分布到有利集合。

建议：

- P1：文档中明确 VDD 的安全语义是概率性验证，并给出 sample count 对检测概率的影响。
- P1：将 sample count、安全参数、文件大小、shard 数的推荐配置固化到文档和脚本。
- P2：后续若用于真实资产交易，需要独立审查 RSLH-VE 的数学假设和实现。

### 6. Oracle 与可用性判断

设计有效点：

- VDD proof 通过后才触发 Oracle。
- Oracle 成功后写入 `oracleSuccessUntil[cCipher]`。
- settle 要求 `oracleSuccessUntil[cCipher] > info.initTime + LIVING_WINDOW`，因此 seller 不能只靠 VDD proof 直接取款。
- Oracle 回调经过 proxy 白名单控制，VDD 只接受配置的 proxy。

实现风险：

- `OracleProxy.request` 把 `bytes cCipher` 直接转成 `string(cCipher)`，但 Walrus consumer JS 逻辑期望参数是 hex string，再转换成 base64url blob id。若 `cCipher` 是任意 bytes32，这个字符串转换链路可能产生无效或不可预期的输入。
- `OracleProxy.handleResponse` 在 err 分支调用 `onOracleError(bytes,bytes)`，而 VDD 当前没有实现该函数。低层 call 失败后 proxy 会发出 CallbackResult(false)，但 VDD 无法记录明确失败状态。
- WalrusFunctionsConsumer 源码中硬编码了 API key。它不是链上私钥，但仍是敏感服务凭证，不适合长期留在源码。
- `OracleProxy.onlyWhitelisted(address caller)` 的参数没有被使用，实际检查的是 `msg.sender`。安全上问题不大，但代码可读性差。

建议：

- P1：统一 `cCipher` 的链上类型和 Oracle 参数编码。若 Walrus blob id 是 hex/base64url 字符串，应显式传 string 或 bytes 编码后在 consumer 中严格解码。
- P2：VDD 增加 oracle error 回调或 proxy 改成通用失败事件，便于调试。
- P2：API key 改为部署参数或后续 setter 配置，源码中不保留真实 key。

### 7. 合约状态机与经济安全

设计有效点：

- `purchase` 锁定 buyer 资金。
- `fulfill` 只能在 deadline 前调用。
- `settle` 要求 VSS、VDD、Oracle 全部满足。
- `refund` 只能在 deadline 后调用。
- `settle` 和 `refund` 都删除 pending exchange，并调整 locked balance，避免重复消费。

关键问题：

- `settle` 没有 `nonReentrant`。当前代码先删状态、减余额，再外部转账，直接重复结算风险较低，但仍建议加上，保持与 `refund` 一致。
- `settle` 和 `refund` 使用 `call{gas: 10_000}`。这对普通 EOA 足够，但对合约钱包、多签或复杂 fallback 可能失败，导致资金卡住。
- sale 可以被多个 buyer purchase。subgraph 当前如果以 sale 维度标记 SETTLED/REFUNDED，会掩盖其他 purchase 的状态。
- listing price 不进合约状态，purchase price 不校验，会导致 UI/subgraph 上的 listing 价格与实际 purchase 金额不一定一致。

建议：

- P2：给 `settle` 加 `nonReentrant`。
- P2：评估是否需要 withdraw pattern 代替固定 gas 转账。
- P2：subgraph 和 UI 以 purchase digest 为主要状态，不要只看 sale status。

### 8. Drop Script 实现与协议意图一致性

当前脚本主流程与协议意图基本一致：

- stage 1 创建或获取 channel，并 listing。
- stage 1.5 提交 data key commitment。
- stage 2 buyer purchase。
- stage 3 seller fulfill，提交 VSS 和 VDD，并触发 Oracle。
- stage 4 buyer recovery。
- stage 5 seller wait oracle and settle。

主要实现问题：

- `get_or_create_channel` 从 receipt 的第一条 log 解码 `ExchangeChannelCreated`，如果 receipt 中有其他 log，解析会不稳。
- `get_purchase_info_from_event` 只寻找 Hub 地址的第一条 log，没有严格过滤 `PurchaseEvent` topic、channel 和 saleId。
- `stage_4_recovery` 使用历史事件最后一条，不能安全支持并发或多订单。
- 脚本当前更像“一次性 happy path 演示程序”，还不是可恢复、可重入、可并发的操作工具。
- 脚本应在 fulfill 前本地检查 `exchange_info.data_commitment`、`sale_digest`、`data_version` 与 listing 一致，尽早发现错误，而不是完全依赖链上 revert。

建议：

- P1：所有 receipt/event 解析必须按 event signature、合约地址、channel、saleId 或 purchase digest 精确过滤。
- P2：为每次 purchase 写入本地 run state，保存 channel、saleId、purchase digest、ephemeral pubkey、tx hash、block number。
- P2：拆分 stage 时必须能从本地 state 或链上事件恢复，不依赖同一进程内存。

### 问题分级

P1，进入真实端到端调试前优先处理：

- `purchase` 未强制 `keccak256(dataCommitment) == dataVersion`。
- deadline 没有合理区间检查。
- VDD 抽样安全语义和参数没有文档化。
- `cCipher` 到 Oracle/Walrus 参数的编码链路需要修正或严格验证。
- receipt/event 解析没有精确过滤。

### 已批准实施方案

#### 1. `purchase` 绑定 `dataCommitment`

在 `ExchangeChannel.purchase` 中，继续保留 `dataVersion == saleVersions[saleId]`，并增加：

```solidity
require(getDataId(dataCommitment) == dataVersion, "Wrong data commitment");
```

这让 buyer 不能再把正确 `dataVersion` 和错误 `dataCommitment` 拆开提交。

#### 2. `purchase` 检查 deadline

增加最小和最大 deadline 窗口：

- `MIN_PURCHASE_DEADLINE = 1 hours`
- `MAX_PURCHASE_DEADLINE = 30 days`

检查规则：

```solidity
require(deadline >= block.timestamp + MIN_PURCHASE_DEADLINE, "Deadline too soon");
require(deadline <= block.timestamp + MAX_PURCHASE_DEADLINE, "Deadline too far");
```

这不是最终产品参数，只是开发调试阶段避免明显无效订单和长期挂起订单。

#### 3. 精确解析 `PurchaseEvent`

`drop-script` 的 purchase receipt 解析改为：

- 只看 Hub 地址发出的 log。
- 只接受能解码为 `PurchaseEvent` 的 log。
- 要求 `event.channel == channel_address`。
- 要求 `event.sale_id == listing.unique_sale_id`。
- 要求 `event.exchange_info.sale_digest == listing.unique_sale_id`。

#### 4. 精确解析 `ExchangeChannelCreated`

`drop-script` 的 channel 创建 receipt 解析改为：

- 只看 Hub 地址发出的 log。
- 只接受能解码为 `ExchangeChannelCreated` 的 log。
- 要求 `event.owner == signer.address()`。
- 要求 Hub 链上 `isRegisteredChannel(event.channel) == true`。

#### 5. Oracle / Walrus `cCipher` 编码链路论证

当前流程中，`cCipher` 的真实语义是 Walrus `BlobId` 的 32 字节原始表示：

- `drop-script` 用 `compute_rs_id(encrypted_asset_data)` 得到 `encrypted_blob_id: [u8; 32]`。
- VDD guest 读取并提交 32 字节 `c_cipher`。
- `drop-lib` 把这 32 字节转换为 `walrus_core::BlobId`。
- `VDD.submitVDDProof` 把 `bytes cCipher` 作为状态 key 和 Oracle 参数。

问题在 Oracle：

- `OracleProxy.request` 当前执行 `args[0] = string(c_cipher)`。
- `WalrusFunctionsConsumer` 的 JS 逻辑把 `args[0]` 当成 hex string，再转 base64url。
- 任意 32 字节不能安全地直接转 Solidity `string`。即使不 revert，也不等价于 hex 编码。

建议方案：

- 保持 VDD 和 ExchangeChannel 中 `cCipher` 类型为 `bytes`，不改证明和状态 key。
- 在 `OracleProxy.request` 内把 `bytes cCipher` 显式转成小写 hex string，带 `0x` 前缀或不带前缀均可，但要和 consumer 约定一致。
- `WalrusFunctionsConsumer` 保留“接受 0x 前缀或无前缀 hex”的逻辑，并增加长度/hex 字符校验。

本轮不实施该改动，因为它会改变 Oracle 合约行为，需要重新部署 OracleProxy / Consumer 相关组件，并重新确认 Chainlink Functions 参数。

### 实施结果

本轮已按用户批准范围完成 1-4：

- `ExchangeChannel.purchase` 增加 `getDataId(dataCommitment) == dataVersion` 检查。
- `ExchangeChannel.purchase` 增加 1 小时到 30 天的 deadline 窗口检查。
- `drop-script` 的 `ExchangeChannelCreated` 解析改为过滤 Hub 地址、事件类型、owner，并检查 channel 已在 Hub 注册。
- `drop-script` 的 `PurchaseEvent` 解析改为过滤 Hub 地址、事件类型、channel、saleId，并检查 `exchangeInfo.saleDigest` 一致。
- 合约测试中原来“挂牌后再取 saleId”的用例被修正为挂牌前锁定 saleId；这是新 data commitment 绑定检查暴露出的旧测试问题。
- 新增三个负向测试，覆盖错误 data commitment、deadline 太短、deadline 太长。

### 部署决策记录

当前 `ExchangeChannel.sol` 已有本轮合约源码修复，但尚未重新部署到 Arbitrum Sepolia。

用户决定：

- 接下来的集成测试继续使用当前已部署版本。
- 如果后续因为其他原因需要重新部署合约，再把本轮 `purchase` 绑定和 deadline 检查改动一起部署上去。
- 在重新部署前，drop-script 集成测试应意识到链上版本还不包含这些新增 purchase 约束。

P2，开发演示可暂缓，但进入可靠工具前应处理：

- listing price 不进合约状态，purchase price 不校验。
- channel 全局 data key commitment 和 buyer privy 状态需要明确产品语义。
- ECIES ephemeral public key 没有持久化。
- `settle` 缺少 `nonReentrant`。
- 固定 10,000 gas 转账可能不兼容合约钱包。
- VSS recovery 从历史事件取最后一条。
- Oracle error 没有进入 VDD 明确状态。
- subgraph sale 状态粒度不足以表达多 purchase。

P3，清理和工程质量：

- 去掉敏感调试日志。
- Walrus consumer API key 不应硬编码在源码。
- `OracleProxy.onlyWhitelisted` 清理未使用参数。
- `delistFile` 对不存在 sale 的错误应更明确。

## 测试验收标准

后续实施迭代应至少满足：

- 合约构建通过：`forge build`。
- drop-lib 测试通过：`cargo test -p drop-lib`。
- VSS/VDD guest 和 script 编译通过。
- `drop-script` 编译通过。
- 单次端到端流程能在 Arbitrum Sepolia 上完成 purchase、fulfill、oracle signal、settle 或 refund。
- subgraph 能索引 channel 创建、listing、purchase、VSS、VDD、settle/refund 事件。
- 所有事件解析按 tx receipt、event topic 和业务 id 精确过滤。
- 不把任何私钥、API key、deploy key 写入 git 或文档。

本轮实施后的实际验证：

- `forge test` 通过：5 个 test suites，18 个 tests 全部通过。
- `PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p drop-script` 通过。
- 独立 `/tmp` target dir 的一次 `cargo check` 因 180 秒 timeout 停在依赖编译阶段，未进入 `drop-script` 错误；最终以原 workspace target dir 的成功结果为准。

## 经验总结

- 这个协议的主干是成立的，但它依赖一组很强的工程假设：单 channel 全局数据密钥、单次 happy path、seller 主动 fulfill、VDD 抽样证明、Oracle availability 门控。
- 下一轮代码修复不应该一口气改完整系统，应先处理会影响协议闭合的 P1：purchase 数据绑定、deadline、Oracle 编码、事件过滤、VDD 安全参数文档。
- VSS/VDD 的 binding hash、guest public values、合约 ABI、script host 输入必须作为同一组接口同步维护；任何一端变化都要更新文档和测试。
- subgraph 只能辅助调试，不能成为安全判断来源。链上状态和 verifier 结果才是结算依据。
