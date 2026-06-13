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

`contracts/src/oracle/` 下包含 Chainlink Functions consumer 和代理合约。VDD 合约通过 `IOracleProxy.request(cCipher, callback)` 发起请求，并通过 `onResponse` 接收状态。

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
| Walrus Functions Consumer | `0xE48eBaB46376A66d5E33B0D02F8BA5AD75580a01` | latest broadcast, block `276651794` |
| Oracle proxy | `0x3919D7EBcef230a049e20C2020da4a4ff7d32754` | latest broadcast, block `276651684` |
| Exchange hub | `0xAd7E0A828D5588e7e75b20244194c55d58c54A0b` | latest broadcast, block `276651753` |
| Exchange logic | `0x6793Bc603a61E37BE89681041C84b68b95291449` | latest broadcast, block `276651719` |

注意：`drop-script` 当前从 `.env` 读取 `HUB_ADDRESS`、`VSS_VERIFIER_ADDRESS`、`VDD_VERIFIER_ADDRESS`，源码常量只作为默认回退。每次重新部署后要同步更新 `contracts/deployed.md`、`drop-script/.env` 和 `subgraph/subgraph.yaml`。

## 最新部署检查

2026-06-13 已完成 Arbitrum Sepolia 部署，并确认以下链上连线：

- `ExchangeHub.implementation()` 指向 `0x6793Bc603a61E37BE89681041C84b68b95291449`。
- `ExchangeHub.oracleWrapper()` 指向 `0x3919D7EBcef230a049e20C2020da4a4ff7d32754`。
- `OracleProxy.consumer()` 指向 `0xE48eBaB46376A66d5E33B0D02F8BA5AD75580a01`。
- `OracleProxy.controller()` 指向 `0xAd7E0A828D5588e7e75b20244194c55d58c54A0b`。
- `OracleProxy.subscriptionId()` 为 `550`。
- `WalrusFunctionsConsumer.proxy()` 指向 `0x3919D7EBcef230a049e20C2020da4a4ff7d32754`。
