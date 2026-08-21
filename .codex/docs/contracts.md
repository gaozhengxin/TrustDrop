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
| VSS | `0x90933a2D8556Bf0785be48D95516238F8C788eBf` | SP1 verifier, block `282520554` |
| VDD | `0x23e85B3d3dCD4597a40CcDE987ac2BA5c7F3481D` | walrus rslh SP1 verifier, block `282520794` |
| Oracle proxy | `0xA79E3d31A95eB1368028ba7b25a2B7b8f56146D9` | hybrid centralized/CRE oracle, block `282682863` |
| Exchange hub | `0xc857542964E8F7618F1A372c36E180D5670b1669` | latest broadcast, block `282682922` |
| Exchange logic | `0xBAA3089aC201AEc7A33B0DE42C1598Af92d9Fc24` | latest broadcast, block `282682879` |

注意：`drop-script` 当前从 `.env` 读取 `HUB_ADDRESS`、`VSS_VERIFIER_ADDRESS`、`VDD_VERIFIER_ADDRESS`，源码常量只作为默认回退。每次重新部署后要同步更新 `contracts/deployed.md`、`drop-script/.env` 和 `subgraph/subgraph.yaml`。

## 最新部署检查

2026-06-30 已完成 Arbitrum Sepolia hybrid oracle 重新部署，并确认以下链上连线：

- `ExchangeHub.implementation()` 指向 `0xBAA3089aC201AEc7A33B0DE42C1598Af92d9Fc24`。
- `ExchangeHub.oracleWrapper()` 指向 `0xA79E3d31A95eB1368028ba7b25a2B7b8f56146D9`。
- `ExchangeHub.vssVerifier()` 仍指向 `0x90933a2D8556Bf0785be48D95516238F8C788eBf`。
- `ExchangeHub.vddVerifier()` 仍指向 `0x23e85B3d3dCD4597a40CcDE987ac2BA5c7F3481D`。
- `OracleProxy.controller()` 指向 `0xc857542964E8F7618F1A372c36E180D5670b1669`。
- `OracleProxy.creForwarder()` 指向 `0x76c9cf548b4179F8901cda1f8623568b58215E62`。
- `OracleProxy.centralizedOracleSigner()` 当前为 `0x5318831f07e8E5e3e8Fdf2a53ef0F0c3996a88dF`。
- `OracleProxy.defaultMode()` 为 `0`，即 centralized。

## 2026-06-30 VSS 复用 view

当前源码和最新部署中的 `VSS` 已新增 VSS 复用辅助 view：

- `needsVSS(address user) -> bool`
- `audienceCount() -> uint256`
- `getAudienceVssKeyCommitments(address[] audiences) -> bytes32[]`

这些函数用于 `drop-cli` / daemon 判断 buyer 是否需要 VSS、构造 batch VSS proof 输入。它们不改变现有 `purchase`、`fulfill`、`settle` 的 ABI，因此旧 `drop-script` full-flow 不受影响。

这次改动不改变现有 `purchase`、`fulfill`、`settle` 的 ABI。重新部署 `ExchangeChannelImplementation`、`ExchangeHub` 和 `OracleProxy` 后，已同步：

- `contracts/deployed.md`
- `drop-script/.env`
- `drop-cli` 使用的 env/profile
- `subgraph/subgraph.yaml` 的 Hub 地址和 start block

`drop-cli` 对旧链上 channel 保留 fallback：优先调用 `needsVSS`，如果旧合约不支持该函数，则退回 `!isPrivy(buyer)`。

## 配置 centralized Oracle signer

在 `contracts/.env` 填本地私钥配置：

```sh
ORACLE_RELAYER_PRIVATE_KEY=<worker signer private key>
ORACLE_PROXY_ADDRESS=0xA79E3d31A95eB1368028ba7b25a2B7b8f56146D9
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
