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
| VSS | `0xE57527263A7e0563e3719D96EfaA82bC3F12E575` | |
| VDD | `0x40379d9685ae004EfC438fd7d40434A7DFd460C5` | walrus rslh |
| Walrus Functions Consumer | `0x4Fe4D01E99DDe0E873D3a35B972009427022d679` | |
| Oracle proxy | `0xbAFb99cD4Cb504225785e4805026f4dFBD6BF427` | |
| Exchange hub | `0x2F0E2DeA5385e8Ea5234ea5c1f46A255fC330b5F` | walrus |
| Exchange logic | `0xc5006FCEeEec398661320Fc7Aa2d374eE351725b` | |

注意：`drop-script/src/main.rs` 当前硬编码的 `HUB_ADDRESS`、`VSS_VERIFIER_ADDRESS`、`VDD_VERIFIER_ADDRESS` 与 `contracts/deployed.md` 记录不完全一致。运行端到端脚本前需要确认使用哪一组部署。

