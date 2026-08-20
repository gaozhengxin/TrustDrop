# 迭代: 替换链上 VDD verifier（新 RSLH-VE Walrus 绑定版）

## 日期
2026-08-20

## 目的
把步骤②完成的 Walrus 绑定版 RSLH-VE fixture（新 vkey）部署为链上 VDD verifier，并让 ExchangeHub 指向新 verifier；其他合约地址不变。不跑 e2e，只做全流程测试的准备检查。

## 链上变更（Arbitrum Sepolia，chainId 421614）
- 新 VDD_RSLH wrapper：`0xd00200365fae479c92aae13fc9b96457b0dd58d6`
  - deploy tx：`0xb51f17aca9be36f1a72fde31b98c380f8f00b8bfa89a9c3055742d8bb2089664`，block `300029968`
  - `verifier()` = SP1 Groth16 gateway `0x397A5f7f3dBd538f23DE225B51f532c34448dA9B`
  - `VDDProgramVKey()` = `0x0079342a118978d5b13a1a5db2bb0f18d5c4902ec440e6a73a4604bfab9da4ab`（新 fixture vkey）
- Hub `setVDDVerifier(0xd002…)`：tx `0x4e5db071371d46bfb5027e6c548be112d1ffb60d368f4cc384dbf795b62b9690`，block `300030797`
- Hub 其余状态不变：owner `0x9396532…AcB89`、VSS `0xCedb1D…63B5A`、implementation `0xB8D465…74c74`、oracle `0x456Eb1…5672`

## 验证
- `forge build/test --root guest/vdd/contracts`：6/6 通过（VDD_RSLH 4 + VDD_filecoin 2），新 fixture 兼容 wrapper。
- 链上只读核验：新 wrapper `verifier()/VDDProgramVKey()` 正确。
- 对已部署 wrapper `eth_call verifyVDD(proof, publicValues, bindingHash)` 返回 `true`（bindingHash 用 fixture cKey 计算；注意 wrapper 绑定的是 cKey 而非 dataKeyCommitment，二者在设计上相等：`dataKeyCommitment == cKey == blake3(asset_key)`）。
- fixture public values 仍为 96 字节 `cOrigin||cKey||cCipher`，cCipher = encrypted blob id（32B），链上 ABI 无需改动。

## 全流程 e2e 准备检查（未执行 e2e）
已就绪：
- [x] guest ELF / fixture / 链上 wrapper vkey 三者一致（ELF sha256 `e42e7c43…`，vkey `0x0079342a…`）。
- [x] VDD 合约 ABI 与 public values 布局未变，旧调用路径（drop-script stage_1_6）签名兼容。
- [x] 绑定语义闭合：`dataKeyCommitment == blake3(key)`，`cCipher == encrypted_blob_id(32B)`。
- [x] 环境配置已更新：`drop-script/.env` VDD_VERIFIER_ADDRESS、`contracts/.env` VDD_ADDRESS、`contracts/deployed.md`。
- [x] 其他合约地址不变 → subgraph 无需重部署；oracle-worker 无需改动。
- [x] `cargo check -p drop-script` 通过。
未就绪（后续步骤）：
- [ ] drop-script `generate_vdd_proof` 仍是旧布局：忽略 walrus_client/blob_id、用旧固定 SYMBOL_SIZE padding；必须改为 `walrus_symbol_size(len)` + 从 Walrus 取真实 blob metadata，用 `build_walrus_blob_opening` 构造打开并写入 guest stdin（否则新 guest 会因缺少/错误 opening 拒证）。
- [ ] drop-cli 更新：vdd 组织参数与传参走 drop-lib 底层库（`walrus_blob_id`/`walrus_open`）。
- [ ] e2e 前需重新创建 channel（channel 在创建时固化 verifier，现存 channel 仍指向旧 verifier）。
- [ ] seller daemon 更新 + 一次完整 live flow（prepare→publish→list→key commitment→VDD prove network→submit→oracle→purchase→fulfill→recover）。

## 注意
- 消耗 PROVE 无需询问；每轮改动先检查再测试。
- 旧 vdd wrapper `0x23fE02beF03588A0E9dD5a4Fd86eE2172205768b` 已不再被 hub 引用，保留在链上无碍。
- foundry 1.7.1 已安装到 Mac mini `~/.foundry/bin`（供合约部署/核验使用）。
