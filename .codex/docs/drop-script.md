# Drop Script 端到端流程

`drop-script` 是当前仓库里把存储、SP1 证明和 Exchange 合约串起来的端到端脚本。入口文件是 `drop-script/src/main.rs`，运行目标是完成一次“卖家挂牌、买家购买、卖家提交 VSS/VDD 证明、Oracle 确认、结算、买家恢复数据”的闭环。

本文按代码现状固化脚本逻辑，并列出要让脚本稳定一次跑通还缺少的集成项。

## 外部依赖

脚本运行依赖以下外部系统：

- Arbitrum Sepolia RPC: 默认 `https://sepolia-rollup.arbitrum.io/rpc`，可由 `ARBITRUM_SEPOLIA_RPC` 覆盖。
- ExchangeHub / ExchangeChannel / VSS / VDD / verifier 合约部署。
- 本地 Walrus daemon: 默认 endpoint 为 `http://localhost:31415`，可由 `WALRUS_LOCAL_ENDPOINT` 覆盖。
- SP1 Prover Network: 使用 `SP1_PRIVATE_KEY` 生成 Groth16 proof。
- 本地输入资产: 默认读取 `drop-script/KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile.mp4`。

关键环境变量：

| 变量 | 用途 |
| --- | --- |
| `SELLER_KEY` | 卖家链上账户私钥 |
| `BUYER_KEY` | 买家链上账户私钥 |
| `SP1_PRIVATE_KEY` | SP1 network 私钥；脚本会写入 `NETWORK_PRIVATE_KEY` |
| `ARBITRUM_SEPOLIA_RPC` | Arbitrum Sepolia RPC，默认 `https://sepolia-rollup.arbitrum.io/rpc` |
| `WALRUS_LOCAL_ENDPOINT` | Walrus publisher/aggregator endpoint，默认 `http://localhost:31415` |
| `HUB_ADDRESS` | ExchangeHub 地址 |
| `VSS_VERIFIER_ADDRESS` | VSS verifier 地址 |
| `VDD_VERIFIER_ADDRESS` | VDD verifier 地址 |
| `DROP_ORACLE_TIMEOUT_SECS` | Oracle 轮询超时，默认 1800 秒 |

当前默认常量：

| 常量 | 当前值 | 用途 |
| --- | --- | --- |
| `INPUT_ASSET_NAME` | `KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile.mp4` | 原始资产文件 |
| `RECOVERED_ASSET_NAME` | `KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile-recovered.mp4` | 买家恢复后的输出文件 |
| `ARBITRUM_SEPOLIA_CHAIN_ID` | `421614` | 交易链 ID |
| `WALRUS_LOCAL_ENDPOINT` | `http://localhost:31415` | Walrus publisher/aggregator fallback |
| `HUB_ADDRESS` | `0x2e506eF3F3cE222F276ddA64Df239CEF92683a78` | ExchangeHub fallback |
| `VSS_VERIFIER_ADDRESS` | `0x5e80ed679fb9f4050a5c7ede5ccbe39178f142a2` | VSS verifier fallback |
| `VDD_VERIFIER_ADDRESS` | `0x154D59Ed30B7784B5c9324b32b9ec5d6c8DE4071` | VDD verifier fallback |

注意：`contracts/deployed.md` 里记录过旧 Hub 地址。调试时优先使用 `contracts/broadcast/DeployMain.s.sol/421614/run-latest.json` 和 `drop-script/.env` 中的地址。

## 代码阶段

### 0. 配置检查

函数：`config_check::run_config_checks`

检查内容：

- `SELLER_KEY`、`BUYER_KEY`、`SP1_PRIVATE_KEY` 是否存在。
- `http://localhost:31415` 是否可连接。
- 卖家、买家、SP1 prover 地址的 ETH 余额。
- VSS/VDD verifier 地址是否有合约代码。

缺口：

- 已检查 Hub 地址是否有代码。
- 没有检查 Hub 中配置的 verifier 是否与脚本硬编码 verifier 一致。
- 没有检查 Walrus publisher 和 aggregator 的具体 API 是否都可用。

### 1. 卖家挂牌

函数：`stage_1_listing`

流程：

1. 读取 `KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile.mp4`。
2. 按 `SYMBOL_SIZE` 对原文补零。
3. 调用 `compute_rs_id` 得到 `original_asset_id`。
4. 使用 `asset_encryption_key` 和 `derive_rslh_nonce(key, b"maenad_v1")` 对补零后的原文做 ChaCha8 加密。
5. 调用 `compute_rs_id` 得到 `encrypted_blob_id`。
6. 上传密文到 Walrus，得到 `walrus_blob_id`。
7. 调用 `get_or_create_channel` 创建 ExchangeChannel。
8. 读取 channel `nonce`，计算 `unique_sale_id = keccak256(channel, chainId, nonce)`。
9. 计算 `onchain_data_version = keccak256(original_asset_id)`。
10. 调用 `channel.listFile(DataCommitment(original_asset_id), price, meta)`。

输出：

- `unique_sale_id`
- `onchain_data_version`
- `walrus_blob_id`
- `channel_address`
- `original_asset_id`
- `encrypted_blob_id`

合约关系：

- `listFile` 内部会记录数据承诺和价格，并写入 sale version。
- `onchain_data_version` 必须与合约 `getDataId(dataCommitment)` 语义一致。当前原始承诺是 32 字节 Walrus/RSLH ID，脚本用 `keccak256(original_asset_id)`，与合约对 bytes commitment 求 keccak 的逻辑匹配。

### 1.5 卖家提交数据密钥承诺

函数：`stage_1_5_submit_key_commitment`

流程：

1. 计算 `data_key_commitment = blake3(asset_encryption_key)`。
2. 调用 `channel.submitDataKeyCommitment(data_key_commitment)`。

合约关系：

- VSS 合约在验证数据密钥分享证明时使用 `dataKeyCommitment`。
- VDD 合约在 `submitVDDProof` 中计算 `bindHash = keccak256(abi.encode(cOrigin, dataKeyCommitment, cCipher))`。

缺口：

- `guest/vdd/script` 的独立测试路径曾使用 `Sha256(key)` 作为 `c_key`，而 `drop-script` 使用 `blake3(key)`。`drop-script` 内部可以自洽，但生产路径应统一 key commitment 算法。
- 当前 VDD guest 读取 `c_key` 并输出它，但没有在 guest 内重新计算并约束 `c_key == hash(key)`。严格安全版本应在 guest 里增加该约束。

### 2. 买家购买

函数：`stage_2_purchase`

流程：

1. 用固定种子 `[0xbb; 32]` 和 `original_asset_id` 派生 `secret_sharing_key`。
2. 使用 seller VSS 私钥派生出的真实 secp256k1 公钥调用 `ecies::encrypt(seller_vss_pub, secret_sharing_key)`。
3. 计算 `vss_key_commitment = blake3(secret_sharing_key)`。
4. 将 `encrypted_vss_key` 作为 `Types.Cipher32`。
5. 调用 `channel.purchase(saleId, dataVersion, price, deadline, dataCommitment, vssKeyCommitment, encryptedVssKey)` 并支付价格。

合约关系：

- 合约记录 pending exchange。
- 买家的 `vssKeyCommitment` 后续会参与 VSS binding hash。

当前缺口：

- `drop-lib::ecies` 输出 32 字节密文和 ephemeral pubkey。合约当前只保存 32 字节 `encryptedVssKey`，没有保存 ephemeral pubkey。
- 当前脚本把 ephemeral pubkey 放在 `PurchaseState` 内存对象里传给履约阶段，因此单次脚本闭环可以真实解密；如果 purchase 和 fulfill 跨进程执行，需要合约字段、事件或链下订单元数据持久化 ephemeral pubkey。

生产补齐方案：

- 把 ephemeral pubkey 写入 purchase 事件、单独映射或链下订单元数据。
- 将 seller VSS 私钥从演示固定值迁移到 `.env` 或安全密钥管理。

### 3. 卖家履约

函数：`stage_3_fulfill`

流程：

1. 从 purchase transaction receipt 解析 `PurchaseEvent`，得到 `buyer` 和 `ExchangeInfo`。
2. 从 channel `audienceList` 读取 buyer 购买时上链的 `encryptedVssKey`。
3. 使用 seller VSS 私钥和 purchase 阶段产生的 ephemeral pubkey 解密出 `secret_sharing_key`。
4. 校验解密结果与 buyer purchase context 中的 `secret_sharing_key` 一致。
5. 用 `secret_sharing_key` 和 nonce `[0u8; 12]` 封装 `asset_encryption_key`，得到 `wrapped_asset_key_vec`。
6. 计算 VSS binding hash，调用 `generate_vss_proof(secret_sharing_key, asset_encryption_key)`。
7. 解析并校验 VSS public values，再调用真实 `verifyVSS(proof, publicValues, bindingHash)` 预验证。
8. 计算 VDD binding hash，调用 `generate_vdd_proof(walrus_blob_id, original_asset_id, encrypted_blob_id)`。
9. 解析并校验 VDD public values，再调用真实 `verifyVDD(proof, publicValues, bindingHash)` 预验证。
10. 调用 `channel.fulfill(buyer, exchange_info, dataVersion, vssArgs, vddArgs)`。

合约关系：

- `fulfill` 会在买家不是 privy 时调用 `shareDataKey`。
- `shareDataKey` 需要 VSS verifier 验证 proof。
- `fulfill` 会在密文未验证时调用 `submitVDDProof`。
- `submitVDDProof` 会调用 VDD verifier，并在通过后触发 Oracle。

当前缺口：

- `simulate_*` 打印了 VK，但没有把 VK 传给 verifier。链上 verifier 通常已经固化 VK；如果 guest 重编译后 VK 变化，必须重新部署 verifier 或更新 verifier 配置。
- `arg_vss.encrypted_data_key` 必须保持 32 字节。当前 asset key 是 32 字节可行；如果未来 asset key 格式变化需要同步改合约和 proof layout。

生产补齐方案：

- 把 verifier 地址从配置读取，并校验 channel 中的 `vssVerifier()` / `vddVerifier()` 与配置一致。

### 4. VDD proof 生成

函数：`generate_vdd_proof`

SP1 guest：`guest/vdd/program-vdd-walrus-rslhve`

Host 输入：

- `original_asset_id`
- `encrypted_blob_id`
- `c_key_bytes = blake3(asset_encryption_key)`
- `aux_data = b"maenad_v1"`
- 私密 `asset_encryption_key`
- `DEFAULT_SAMPLE_COUNT` 组 RSLH-VE shard proof

Guest 公开输出：

```text
c_origin_bytes || c_key || c_cipher_bytes
```

合约绑定：

```solidity
bindHash = keccak256(abi.encode(cOrigin, dataKeyCommitment, cCipher))
```

应满足：

- `publicValues[0..32] == cOrigin`
- `publicValues[32..64] == dataKeyCommitment`
- `publicValues[64..96] == cCipher`
- `cOrigin == info.dataCommitment`
- `cCipher == vdd.cCipher`

当前缺口：

- 脚本已经在提交前解析 VDD public values 并检查 `c_origin`、`c_key`、`c_cipher` 三个字段。
- Guest 内仍没有重新计算 `blake3(asset_encryption_key)`，无法单独约束 `c_key` 与私钥一致。

### 5. VSS proof 生成

函数：`generate_vss_proof`

SP1 guest：`guest/vss/program`

Host 输入：

- `length = 1`
- `message = asset_encryption_key`
- `watcher/shared key = secret_sharing_key`
- `nonce = [0u8; 12]`

公开输出由 VSS guest 按 ABI 风格拼接，包含：

- 消息 hash
- encrypted data key
- key commitment
- nonce

合约绑定：

VSS 合约会把 `dataKeyCommitment`、每个 audience 的 `vssKeyCommitment` 和 `encryptedDataKeys` 编进 binding hash，再交给 verifier。

当前状态：

- 脚本会解析 VSS public values，并检查 encrypted data key、key commitment、nonce。
- secret sharing key 会从 buyer 上链的 `encryptedVssKey` 解密得到。
- 当前限制是 ephemeral pubkey 仍依赖单次脚本内存上下文。

### 6. 等待 Oracle

函数：`wait_for_oracle_signal`

流程：

1. 轮询 `channel.oracleSuccessUntil(cCipher)`。
2. 如果 `successUntil > now`，认为 Oracle 已经确认密文可用。
3. 否则每 15 秒重试。

当前缺口：

- 已增加 `DROP_ORACLE_TIMEOUT_SECS`，默认 1800 秒。
- 当前检查 `successUntil > now`，settle 阶段仍由合约最终检查 `successUntil > info.initTime + LIVING_WINDOW`。

生产补齐方案：

- 如果需要更早失败，可在脚本侧也检查 `successUntil > info.initTime + LIVING_WINDOW`。

### 7. 结算

函数：`stage_5_settle`

流程：

1. 重新从 purchase transaction 解析 `buyer` 和 `ExchangeInfo`。
2. 调用 `channel.settle(buyer, info, dataVersion, cCipher)`。

合约条件：

- pending exchange 存在。
- 买家已被 `shareDataKey` 标记为 privy。
- VDD proof 已验证。
- Oracle 对 `cCipher` 的成功窗口有效。

### 8. 买家恢复数据

函数：`stage_4_recovery`

流程：

1. 监听 `DataKeyShared(address[],bytes32[])`。
2. 找到当前 buyer 地址对应的位置。
3. 用 `secret_sharing_key` 解密 `encrypted_data_keys[pos]`，得到 `asset_key`。
4. 从 Walrus 下载密文。
5. 用 `derive_rslh_nonce(asset_key, b"maenad_v1")` 解密密文。
6. 写入 `KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile-recovered.mp4`。

当前缺口：

- 代码已改为监听本次 `channel_address` 上的 `DataKeyShared`，并按 stage 1 保存的 `original_len` 裁剪 padding。
- 事件查找仍是查询 channel 上最后一条 `DataKeyShared`，更严格的实现应从本次 fulfill receipt 精确解析。

生产补齐方案：

- 从 `fulfill` receipt 精确解析 `DataKeyShared`，不要扫全局最后一条日志。

## 一次跑通检查清单

运行前应确认：

- `drop-script`、`drop-lib`、VSS/VDD guest 和 script 均可编译。
- `drop-lib` 测试通过。
- `guest/vss/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/vss-program` 存在。
- `guest/vdd/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/program-vdd-walrus-rslhve` 存在。
- Hub、channel implementation、VSS verifier、VDD verifier 是同一轮部署产物。
- 链上 verifier 使用的 VK 与当前 guest ELF 对应。
- Walrus daemon 可上传和下载。
- centralized Oracle Worker / OracleProxy 可正常回调。
- 卖家和买家地址有足够 Arbitrum Sepolia ETH。
- SP1 prover network 账户可提交 Groth16 proof。

## Centralized Oracle Worker

`drop-script` 已支持在 fulfill 后主动触发 centralized Oracle Worker，但这是显式 opt-in：

```sh
ORACLE_MODE=centralized
ORACLE_WORKER_URL=https://trustdrop-oracle-worker.zhengxingao.workers.dev
ORACLE_WORKER_TOKEN=...
ORACLE_WORKER_STATUS_URL=https://trustdrop-oracle-worker.zhengxingao.workers.dev/status
```

规则：

- 未设置 `ORACLE_MODE` 或 `ORACLE_MODE=external` 时，`drop-script` 不触发 Worker，只按旧逻辑等待 `oracleSuccessUntil`。
- `ORACLE_MODE=centralized` 时，fulfill 成功后会先请求 Worker `/status`；只有 `ok=true` 才请求 `/oracle/fulfill`。
- Worker 返回 `reportTxHash` 后，脚本继续轮询 `oracleSuccessUntil`。
- Worker 已部署到 `https://trustdrop-oracle-worker.zhengxingao.workers.dev`，当前真实 `.env` 已启用 `ORACLE_MODE=centralized`。

## 当前状态结论

代码已迁到 `drop-script` / `drop-lib` 命名，并能在 SP1 v6 依赖下编译。`drop-script` 目前仍是端到端演示脚本，不是完整生产集成。最关键的缺失是：

1. ephemeral pubkey 仍未链上或链下持久化，purchase/fulfill 跨进程时需要补元数据。
2. VDD key commitment 已在脚本侧统一为 `blake3`，但 guest 内仍应增加 `c_key == blake3(key)` 约束。
3. `DataKeyShared` 已监听 channel 并裁剪 padding，但最好从 fulfill receipt 精确解析。
4. Oracle 已有超时，部署地址仍需配置化并校验 channel 内 verifier。
