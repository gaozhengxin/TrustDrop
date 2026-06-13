# Drop Script 集成调试依赖关系手册

## 目的

本手册用于管理 `drop-script` 后续每一次集成调试的依赖关系。重点不是解释协议细节，而是回答：

- 调试前需要准备哪些组件。
- 某个组件变化后，哪些东西必须重新编译、重新部署或重新配置。
- 哪些步骤可以由 Codex 执行。
- 哪些步骤必须由用户手动处理或确认。
- 每次调试开始前，必须列出哪些人工调整项。

## 当前原则

- 当前集成测试继续使用已部署的 Arbitrum Sepolia 合约。
- 本地源码中的 `purchase` 绑定和 deadline 检查已经完成，但尚未部署到链上。
- 如果后续需要重新部署合约，再把这次合约改动一起部署。
- Oracle / Walrus `cCipher` 编码问题已经识别，但尚未实施修复；后续调试如果卡在 Oracle，应优先处理它。
- VDD 抽样安全参数本轮不处理。

## 调试依赖总图

```text
guest/vss, guest/vdd
  -> ELF / proving key / verification key
  -> verifier contracts
  -> contracts deployment
  -> drop-script verifier address env

contracts/src
  -> forge build/test
  -> deploy ExchangeHub / OracleProxy / Consumer / Channel implementation / verifiers
  -> drop-script HUB_ADDRESS and verifier env
  -> subgraph ABI, manifest address, startBlock

OracleProxy + WalrusFunctionsConsumer + Chainlink Functions
  -> Chainlink subscription, router, DON id, consumer allowlist
  -> OracleProxy consumer config
  -> Hub / Channel deployment
  -> VDD oracleSuccessUntil
  -> settle

Walrus publisher
  -> encrypted blob upload
  -> Walrus BlobId
  -> VDD cCipher
  -> Oracle availability query

subgraph
  -> Hub address and startBlock
  -> ABI and event mappings
  -> Studio deployment
  -> observability only, not settlement security

drop-script/.env
  -> keys, RPC, Walrus endpoint, Hub, verifier addresses
  -> all runtime stages
```

## 组件依赖表

| 组件 | 依赖 | 被谁依赖 | 变化后动作 |
| --- | --- | --- | --- |
| `guest/vss/program` | `drop-lib`, SP1 version | VSS script, VSS verifier contract | 重新 build guest，重新生成 VK，重新部署 VSS verifier，更新 `drop-script/.env` |
| `guest/vdd/program-vdd-walrus-rslhve` | `drop-lib::rslh_ve`, SP1 version, Walrus BlobId 语义 | VDD script, VDD verifier contract | 重新 build guest，重新生成 VK，重新部署 VDD verifier，更新 `drop-script/.env` |
| `drop-lib` | RSLH-VE、ECIES、KDF、CID/BlobId 工具 | guest、script、drop-script | 跑 `cargo test -p drop-lib`，再按受影响 guest/script 重编 |
| `contracts/src/ExchangeChannel.sol` | VSS/VDD/Oracle ABI | deployed Hub/channel implementation, subgraph ABI | 跑 `forge test`；若要链上生效，重新部署合约并更新 env/subgraph |
| `contracts/src/VSS.sol` | VSS verifier ABI/public values | Channel fulfill, DataKeyShared | 重新部署合约；必要时更新 subgraph ABI/mapping |
| `contracts/src/VDD.sol` | VDD verifier ABI、OracleProxy ABI | Channel fulfill/settle, Oracle callback | 重新部署合约；必要时更新 Oracle 配置和 subgraph ABI/mapping |
| `contracts/src/oracle/*` | Chainlink Functions router/subscription/DON/API key | VDD availability, settle | 重新部署或重新配置 Oracle；用户手动确认 Chainlink 配置 |
| `drop-script/src/main.rs` | ABI、env、Walrus、SP1 SDK | 端到端调试 | `cargo check -p drop-script`；如 ABI 改变先更新 SDK ABI |
| `sdk/src/abi.rs` | 合约 ABI | drop-script | 合约 ABI 变更后更新；再 `cargo check -p drop-script` |
| `subgraph/` | deployed Hub address、ABI、events | 调试观察 | codegen/build/deploy Studio；不影响链上安全 |
| `drop-script/.env` | 私钥、RPC、合约地址、verifier 地址 | drop-script runtime | 用户或 Codex 按地址更新；不得提交 |
| `contracts/.env` | 部署私钥、RPC、Chainlink 参数 | forge deploy script | 用户维护私钥和外部账号配置；不得提交 |
| `subgraph/.env` | Studio slug、deploy key | subgraph deploy | 用户维护 deploy key；不得提交 |

## 改动触发矩阵

### 改 guest 程序

触发条件：

- 修改 `guest/vss/program/src/main.rs`
- 修改 `guest/vdd/program-vdd-walrus-rslhve/src/main.rs`
- 修改影响 guest 逻辑的 `drop-lib`
- 升级 SP1

必须做：

- `PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p vss-program`
- `PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p program-vdd-walrus-rslhve`
- 重新 build ELF。
- 重新生成或确认 verification key。
- 重新部署对应 verifier 合约。
- 更新 `drop-script/.env` 的 verifier 地址。
- 如果 Hub 构造参数引用 verifier 地址，也需要重新部署 Hub / Channel implementation。

需要用户手动确认：

- 是否允许使用 SP1 Prover Network。
- SP1 private key / prover balance 是否可用。
- 是否接受重新部署 verifier 和主合约。

### 改合约业务逻辑

触发条件：

- 修改 `ExchangeHub.sol`
- 修改 `ExchangeChannel.sol`
- 修改 `VSS.sol`
- 修改 `VDD.sol`
- 修改 interfaces 或 Types

必须做：

- `forge build`
- `forge test`
- 判断是否需要重新部署。
- 如果重新部署，更新：
  - `contracts/deployed.md`
  - `drop-script/.env`
  - `subgraph/subgraph.yaml`
  - subgraph startBlock
  - `.codex/docs/contracts.md`

需要用户手动确认：

- 是否使用 `contracts/.env` 中的部署私钥。
- 是否消耗 Arbitrum Sepolia ETH。
- 是否要把当前源码中的未部署修复一起部署。
- 是否需要重新配置 Chainlink Functions consumer / subscription。

### 改 Oracle / Chainlink Functions

触发条件：

- 修改 `OracleProxy.sol`
- 修改 `FunctionsConsumer_Walrus.sol`
- 修改 Walrus API key、router、DON id、subscription id
- 修改 `cCipher` 编码规则

必须做：

- `forge build`
- `forge test`
- 重新部署 OracleProxy / WalrusFunctionsConsumer，或调用 setter 更新配置。
- 确认 OracleProxy whitelist 包含 VDD/channel。
- 确认 consumer proxy 指向 OracleProxy。
- 确认 Chainlink subscription 有余额，且 consumer 已加入 subscription。
- 调一次独立 Oracle 请求或在 drop-script fulfill 后检查 `oracleSuccessUntil(cCipher)`。

需要用户手动处理：

- 在 Chainlink Functions 控制台确认 subscription。
- 给 subscription 充值测试 LINK 或确认余额。
- 把 consumer 加入 subscription allowlist。
- 确认 Blockberry / Walrus API key 是否可用。
- 如果 API key 要保密，由用户通过 setter 配置，不写入 git。

当前已知 Oracle 风险：

- `cCipher` 是 32 字节 Walrus BlobId 原始字节。
- `OracleProxy.request` 当前把 bytes 直接转 string。
- `WalrusFunctionsConsumer` JS 按 hex string 处理。
- 后续修复建议是在 OracleProxy 内把 bytes 显式 hex 编码，再传给 consumer。

### 改 Walrus publisher 或存储环境

触发条件：

- 修改 `/home/justin/walrus/start.sh`
- 修改 Walrus endpoint
- 切换 Walrus mainnet/testnet/context
- 切换 publisher 或 aggregator

必须做：

- 启动 `/home/justin/walrus/start.sh`。
- 确认 `drop-script/.env` 的 `WALRUS_LOCAL_ENDPOINT`。
- 上传一个小样本，确认返回 blob id。
- 确认 Oracle 查询的网络能看到同一个 blob。

需要用户手动处理：

- 确认 Walrus 私钥 / Sui wallet 可用。
- 确认 Walrus 存储账户余额或配额。
- 如果 publisher 需要登录、授权或资金，由用户处理。

### 改 subgraph

触发条件：

- 合约地址变了。
- startBlock 变了。
- 事件 ABI 变了。
- mapping/schema 变了。

必须做：

- `pnpm --dir subgraph codegen`
- `pnpm --dir subgraph build`
- `pnpm --dir subgraph deploy:studio`
- 记录新 Studio version 和 query URL。

需要用户手动处理：

- 确认 `subgraph/.env` 的 deploy key 有效。
- 如 Studio 项目变更，提供新的 slug。

### 改 drop-script 流程

触发条件：

- 修改 stage 顺序。
- 修改事件解析。
- 修改 env 变量名。
- 修改 purchase/fulfill/settle 参数。
- 修改 proof public values 或 binding hash。

必须做：

- `PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p drop-script`
- 如果 ABI 变化，先更新 `sdk/src/abi.rs`。
- 如果 proof 输入变化，重新检查 guest public values 和合约 verifier ABI。
- 如果链上参数变化，更新 `.codex/docs/drop-script.md`。

需要用户手动确认：

- 是否允许使用 `.env` 中真实私钥发交易。
- 是否允许调用 SP1 Prover Network。
- 是否允许访问本地 Walrus publisher。

## 每次调试前必须输出的人工处理清单

每次开始集成调试前，Codex 必须先列出本轮需要用户手动确认或处理的事项。模板如下：

```md
## 本次调试人工处理清单

- [ ] 是否使用当前已部署合约地址？
- [ ] 如果需要重部署，是否允许使用 `contracts/.env` 私钥？
- [ ] 是否需要重新配置 Chainlink Functions subscription / consumer？
- [ ] Chainlink subscription 是否有余额？
- [ ] Walrus publisher 是否已启动？
- [ ] Walrus/Sui 钱包是否有足够余额或配额？
- [ ] 是否允许使用 `drop-script/.env` 的 seller/buyer/SP1 私钥？
- [ ] SP1 Prover Network key 是否有余额和 allowance？
- [ ] 是否需要重新部署 subgraph？
- [ ] 是否需要更新前端或查询 URL？
```

如果某项不相关，必须写明“不相关”的原因，而不是省略。

## 每次调试前机器检查清单

Codex 可执行的检查：

```sh
git status --short
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p drop-script
forge test
pnpm --dir subgraph build
```

按需执行：

```sh
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p vss-program
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p program-vdd-walrus-rslhve
PROTOC=/tmp/protoc-25.3/bin/protoc cargo test -p drop-lib
```

链上只读检查：

```sh
source contracts/.env
cast block-number --rpc-url "$ARBITRUM_SEPOLIA_RPC_URL"
cast code <CONTRACT_ADDRESS> --rpc-url "$ARBITRUM_SEPOLIA_RPC_URL"
```

## 部署地址同步规则

任何重新部署都必须同步以下位置：

- `contracts/deployed.md`
- `contracts/broadcast/.../run-latest.json`
- `drop-script/.env`
- `subgraph/subgraph.yaml`
- `.codex/docs/contracts.md`
- `.codex/docs/drop-script-debug-plan.md`
- 本 runbook 的当前状态部分，如有地址变化

不能同步的内容：

- 私钥
- API key
- deploy key
- Chainlink subscription secret

## Oracle 专项调试顺序

如果 `fulfill` 成功但 `settle` 卡住，按以下顺序查：

1. `vddVerified(cCipher)` 是否为 true。
2. `lastOracleRequestAt(cCipher)` 是否更新。
3. OracleProxy 是否发出 `RequestSent`。
4. WalrusFunctionsConsumer 是否收到 Chainlink Functions 回调。
5. OracleProxy 是否发出 `CallbackResult`。
6. VDD `oracleSuccessUntil(cCipher)` 是否大于 `initTime + LIVING_WINDOW`。
7. Walrus API 是否能查到对应 blob。
8. `cCipher` 是否按 hex string 正确传给 consumer。

若第 8 项失败，优先修复 OracleProxy 的 bytes-to-hex 编码。

## 当前未部署但已存在的源码改动

截至 0002 收口，以下源码改动已提交，但当前 Arbitrum Sepolia 部署不一定包含：

- `purchase` 强制 `getDataId(dataCommitment) == dataVersion`。
- `purchase` deadline 限制为 1 小时到 30 天。
- `drop-script` 精确解析 channel created 和 purchase event。

调试时必须区分：

- 本地源码行为。
- 当前链上部署行为。

如果需要验证这些合约约束，必须重新部署合约。
