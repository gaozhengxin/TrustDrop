# 合约与部署

## 合约职责

### `ExchangeHub`

- 保存 channel implementation 地址。
- 通过 OpenZeppelin Clones 创建 `ExchangeChannel`。
- 记录合法 channel。
- 聚合发出 `SaleListed`、`PurchaseEvent`、`SettleEvent`、`RefundEvent` 等事件。

### `ExchangeChannel`

核心交易状态机：

- `listFile`: 卖家挂牌数据承诺和价格。
- `updateFile`: 更新销售的数据版本。
- `delistFile`: 下架。
- `purchase`: 买家付款并提交 VSS 相关承诺。
- `fulfill`: 卖家提交 VSS/VDD proof，分发数据密钥并触发 Oracle。
- `settle`: 条件满足后结算给卖家。
- `refund`: deadline 后未履约时退款给买家。

### `VSS`

- 管理 audience 注册。
- 保存 `dataKeyCommitment`。
- 使用 VSS verifier 验证数据密钥封装 proof。
- 通过 bucket bitmap 标记买家是否已经 privy。

### `VDD`

- 维护数据承诺、引用计数和 VDD proof 状态。
- 使用 VDD verifier 验证 `proof/publicValues/bindHash`。
- proof 通过后触发 Oracle 检查密文可用性。
- 保存 `oracleSuccessUntil[cCipher]`。

### Oracle

`contracts/src/oracle/` 下当前使用 hybrid Oracle proxy。VDD 合约通过 `IOracleProxy.request(cCipher, callback)` 发起请求；中心化 Worker 后续调用 `submitCentralizedReport`，CRE 路径后续调用 `onReport`，两者最终都通过 `onResponse` 更新 VDD 状态。

## 关键事件

- `ExchangeChannelCreated(owner, channel)`
- `SaleListed(channel, saleId, dataCommitment, price, version, info)`
- `PurchaseEvent(channel, saleId, dataCommitment, buyer, price, exchangeInfo)`
- `DataKeyShared(audiences, encryptedDataKeys)`
- `VDDProofSubmitted(cCipher)`
- `SettleEvent(channel, buyer, saleId, dataCommitment)`
- `RefundEvent(channel, buyer, saleId, dataCommitment, amount)`

## 已记录部署地址

`contracts/deployed.md` 记录了 Arbitrum Sepolia 地址：

| 合约 | 地址 | 备注 |
| --- | --- | --- |
| VSS | `0x5e80ed679fb9f4050a5c7ede5ccbe39178f142a2` | SP1 verifier |
| VDD | `0x154D59Ed30B7784B5c9324b32b9ec5d6c8DE4071` | walrus rslh SP1 verifier |
| Oracle proxy | `0x13A59912Fe91211FB7a901974997F716f11EcFe8` | hybrid centralized/CRE oracle, block `280261101` |
| Exchange hub | `0x1C01E8E981909926Ed67B5eEfAbfDfeCAcC882a1` | latest broadcast, block `280261185` |
| Exchange logic | `0xAf34AE4156d304f8C65F5Fa211A9005B0477bbd6` | latest broadcast, block `280261144` |

注意：`drop-script` 当前从 `.env` 读取 `HUB_ADDRESS`、`VSS_VERIFIER_ADDRESS`、`VDD_VERIFIER_ADDRESS`，源码常量只作为默认回退。每次重新部署后要同步更新 `contracts/deployed.md`、`drop-script/.env` 和 `subgraph/subgraph.yaml`。

## 最新部署检查

2026-06-23 已完成 Arbitrum Sepolia hybrid oracle 部署，并确认以下链上连线：

- `ExchangeHub.implementation()` 指向 `0xAf34AE4156d304f8C65F5Fa211A9005B0477bbd6`。
- `ExchangeHub.oracleWrapper()` 指向 `0x13A59912Fe91211FB7a901974997F716f11EcFe8`。
- `ExchangeHub.vssVerifier()` 仍指向 `0x5e80ed679fb9f4050a5c7ede5ccbe39178f142a2`。
- `ExchangeHub.vddVerifier()` 仍指向 `0x154D59Ed30B7784B5c9324b32b9ec5d6c8DE4071`。
- `OracleProxy.controller()` 指向 `0x1C01E8E981909926Ed67B5eEfAbfDfeCAcC882a1`。
- `OracleProxy.creForwarder()` 指向 `0x76c9cf548b4179F8901cda1f8623568b58215E62`。
- `OracleProxy.centralizedOracleSigner()` 当前为 `0x5318831f07e8E5e3e8Fdf2a53ef0F0c3996a88dF`。
- `OracleProxy.defaultMode()` 为 `0`，即 centralized。

## 配置 centralized Oracle signer

在 `contracts/.env` 填本地私钥配置：

```sh
ORACLE_RELAYER_PRIVATE_KEY=<worker signer private key>
ORACLE_PROXY_ADDRESS=0x13A59912Fe91211FB7a901974997F716f11EcFe8
```

然后运行：

```sh
cd contracts
set -a
source .env
set +a
forge script script/SetCentralizedOracleSigner.s.sol:SetCentralizedOracleSigner \
  --rpc-url https://sepolia-rollup.arbitrum.io/rpc \
  --broadcast
```

该脚本用 `PRIVATE_KEY` 作为合约 owner/deployer 发交易，从 `ORACLE_RELAYER_PRIVATE_KEY` 推导 signer 地址并写入 `OracleProxy.setCentralizedOracleSigner`。不要把 oracle 私钥提交到 git 或发到聊天里。

当前已配置：

- signer: `0x5318831f07e8E5e3e8Fdf2a53ef0F0c3996a88dF`
- funding tx: `0x27ac6a4408eedc521854c3871050d6794c71cd99f9c913656a1b13d16b51e6c1`
- signer config tx: `0xc49b7f1398c95e52b14c34285452c8142255ef0576ed0aaed94c6f6c8cb50dd8`
