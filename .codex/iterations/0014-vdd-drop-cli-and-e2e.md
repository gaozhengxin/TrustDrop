# 迭代: drop-script/drop-cli 接入新 VDD guest + 线上 e2e 跑通

## 日期
2026-08-20

## 目的
步骤②改了 guest（Walrus 绑定 + 动态符号尺寸），但 drop-script 的 `generate_vdd_proof` 还是旧布局（固定 SYMBOL_SIZE padding、完全忽略 blob opening），drop-cli 也继续引用已移除的 `SYMBOL_SIZE`。本轮把 seller 侧调用代码对齐新 guest，并在容器内编译后完成一次线上 e2e。

## 代码改动（dev 分支，未 push）
- `drop-script/src/main.rs`
  - `generate_vdd_proof` 重写：动态 `walrus_symbol_size(len)`；本地重构密文后调用 `build_cipher_blob_opening` 构造 Walrus 承诺打开并断言 `opening.blob_id == cCipher`；stdin 写入顺序与新 guest 一致（5 个标量 → 15 组采样 → opening）。
  - `stage_1_listing` 移除旧的 1024 倍数 padding（与参考实现一致，直接使用原始字节）。
  - 移除 `SYMBOL_SIZE` 导入，新增 `walrus_symbol_size`/`build_cipher_blob_opening`。
- `drop-cli/src/main.rs`
  - `asset_prepare` 移除 padding；移除 `SYMBOL_SIZE` 导入。

## 构建/部署
- 在 `trustdrop-seller-daemon` 容器内 `cargo check -p drop-script -p drop-cli` 与 `cargo build -p drop-cli` 通过。
- `docker restart trustdrop-seller-daemon` 让 daemon 使用新二进制。

## 线上 e2e 结果（Arbitrum Sepolia，hubble-deep-field.jpg 459864 字节）
- `phase prepare` → sale `0xfc7ec81e...`；`phase publish --yes`
  - Walrus blob：`YeXzKYX3ymhIwySdsAl6Y5UsQ0hCCLuX15FJpMUj6fY`（end epoch 41）
  - 新 channel：`0xe77c043553bdf9063c27e4eca304e9b090db6330`（创建于 verifier 替换之后，vddVerifier=`0xd00200365fAe479c92aAe13Fc9B96457B0dD58D6`）
  - listFile tx `0x80fe63e5...`；submitDataKeyCommitment tx `0x355f5c8c...`
- `proof vdd --yes`（新代码 + prove network）
  - VK `0x0079342a118978d5b13a1a5db2bb0f18d5c4902ec440e6a73a4604bfab9da4ab`（新 vkey），symbol_size=4
  - 链上模拟 `eth_call` 对新 verifier `0xd002...` 验证通过
  - submitVDDProof tx `0x222aae642544ce9da24045c865ee088723683c1099ad4b3e12b2698de7ca675d`
  - oracle worker report tx `0x3ef6d1761c23a2f16cc226ff1a37e5daf6e003d07c95cd97949b520b5aee976f`
- `phase complete-test-flow --yes`：VDD 已验证明短路（不重复消耗 PROVE）→ oracle pulse → 买家购买 → VSS 证明（prove network，VK `0x002e5cf8...`，链上模拟通过）→ fulfill → settle → recovery
- 链上终态核验（cast 只读）：
  - channel.vddVerifier = 新 wrapper；vddVerified(cCipher)=true；oracleSuccessUntil=1791244800；isPrivy(buyer)=true
  - 恢复文件 SHA256 == 原始 hubble-deep-field.jpg SHA256（`5d15d4a2...`）

## 备注
- 恢复文件名仍是旧常量（`KSC-19690716-...-mobile-recovered.mp4`），内容正确，命名是历史遗留。
- 每次运行都会触发一次“blob unavailable/expired → 重新上传”的路径（聚合器状态查询与上传判定的既有行为），Walrus 端去重不产生新 blob，历史遗留，后续可修。
- e2e 使用的 BUYER_KEY 与管理员地址一致（环境中的测试用键设计）。
