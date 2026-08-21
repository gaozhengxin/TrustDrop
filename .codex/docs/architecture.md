# 架构与协议流程

## 组件边界

```text
买家 / 卖家脚本
    │
    ├── drop-lib: 加密、哈希、CID、RSLH-VE 抽样验证
    ├── storage: Walrus / Filecoin 上传下载与状态查询
    ├── SP1 prover: 生成 VSS / VDD Groth16 proof
    │
    └── EVM 合约
        ├── ExchangeHub: channel 工厂与事件聚合
        ├── ExchangeChannel: 挂牌、购买、履约、结算、退款
        ├── VSS: 受众注册、数据密钥分发证明
        ├── VDD: 数据解密证明与 Oracle 请求
        └── OracleProxy / FunctionsConsumer: 存储可用性检查
```

## 端到端流程

当前完整流程位于 `drop-script/src/main.rs`。更细的阶段输入输出、合约调用和缺失集成项见 [Drop Script 端到端流程](./drop-script.md)。

1. 卖家挂牌 `stage_1_listing`
   - 读取 `KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile.mp4`。
   - 按 `SYMBOL_SIZE` 填充。
   - 计算 `originalAssetId`。
   - 用 `assetEncryptionKey` 和 RSLH nonce 加密原文。
   - 上传密文到 Walrus，得到 `walrusBlobId`。
   - 创建交易通道，生成 `saleId`。
   - 调用 `listFile` 上链挂牌。

2. 卖家提交密钥承诺 `stage_1_5_submit_key_commitment`
   - 对资产加密密钥做 `blake3`。
   - 调用 `submitDataKeyCommitment` 固化承诺。

3. 买家购买 `stage_2_purchase`
   - 派生 `secretSharingKey`。
   - 使用卖家 VSS 公钥加密该共享密钥。
   - 提交 `vssKeyCommitment`、价格、deadline 和数据承诺。
   - 合约锁定买家付款并记录 pending exchange。

4. 卖家履约 `stage_3_fulfill`
   - 从 `PurchaseEvent` 解析 buyer 和 `ExchangeInfo`。
   - 用 `secretSharingKey` 封装真实数据密钥。
   - 生成 VSS proof，证明封装密钥正确。
   - 生成 VDD proof，证明密文与原文、密钥承诺一致。
   - 调用 `fulfill`，合约验证 proof 并触发 Oracle。

5. 等待 Oracle `wait_for_oracle_signal`
   - 轮询 `oracleSuccessUntil[cCipher]`。
   - Oracle 成功后合约记录密文可用窗口。

6. 结算 `stage_5_settle`
   - `settle` 检查买家已 privy、VDD 已验证、Oracle 可用性未过期。
   - 合约把锁定资金转给卖家。

7. 买家恢复 `stage_4_recovery`
   - 从 `DataKeyShared` 事件取加密数据密钥。
   - 用 `secretSharingKey` 解出资产密钥。
   - 下载 Walrus 密文并解密为 `KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile-recovered.mp4`。

## Proof 公开值

### VSS guest

位置: `guest/vss/program/src/main.rs`

输入包括消息、多个密钥和 nonce。公开输出按 ABI 风格拼接：

- `length`
- `blake3(msg)`
- ciphertext 动态数组
- key commitment 动态数组
- nonce 动态数组

### VDD RSLH-VE guest

位置: `guest/vdd/program-vdd-walrus-rslhve/src/main.rs`

输入包括：

- `c_origin_bytes`
- `c_cipher_bytes`
- `c_key`
- `aux_data`
- 私密 `key`
- `DEFAULT_SAMPLE_COUNT` 组抽样 shard proof

公开输出拼接为：

- `c_origin_bytes`
- `c_key`
- `c_cipher_bytes`
