# 项目总览

## 项目定位

Maenad / TrustDrop 是一个面向数字资产交易的验证型交付原型。它把数据资产加密上传到去中心化存储，买家付款后，卖家通过零知识证明和链上合约证明两件事：

- VSS: 买家能拿到正确封装的数据密钥。
- VDD: 链下存储的密文确实由约定原文和数据密钥生成。

合约再结合 Oracle 对密文可用性的检查，决定卖家结算或买家退款。

## 技术栈

- Rust workspace: 核心库、SDK、脚本、存储客户端、Walrus core、Tauri 后端。
- Solidity / Foundry: `ExchangeHub`、`ExchangeChannel`、`VSS`、`VDD`、Oracle 相关合约。
- SP1 zkVM: VSS/VDD guest 程序和 EVM 可验证 proof。
- Ethers-rs: Rust 脚本调用 EVM 合约。
- Walrus / Lighthouse Filecoin: 去中心化存储上传、下载、状态查询。
- Tauri + Vite + TypeScript: GUI 目前仍是默认 greet 示例形态。

## 仓库结构

```text
.
├── app/gui/              # Tauri 2 + Vite 前端，当前是原型壳
├── contracts/            # Foundry Solidity 合约、测试、部署脚本、flattened 合约
├── guest/
│   ├── fibo3/            # SP1 示例工程
│   ├── vdd/              # VDD SP1 guest、脚本和验证合约模板
│   └── vss/              # VSS SP1 guest、脚本和验证合约模板
├── drop-lib/           # 密码学、承诺、CID、Merkle、RSLH-VE 等通用逻辑
├── drop-script/        # 端到端链上/链下演示流程
├── sdk/                  # ABI、证明、Walrus 辅助封装
├── storage/              # StorageNetwork 抽象、Walrus/Filecoin 客户端和 CLI
├── walrus-core/          # 本地 vendored/裁剪的 Walrus core no_std 代码
└── docs/                 # 本文档目录
```

## 核心概念

- `saleId`: `keccak256(channel, chainId, nonce)`，标识一次挂牌销售。
- `dataVersion`: 合约侧使用 `keccak256(dataCommitment)` 管理数据版本。
- `originalAssetId`: 当前脚本中通过 `compute_rs_id` 对填充后的原文计算得到。
- `encryptedBlobId`: 对加密后数据计算得到的承诺。
- `dataKeyCommitment`: 对资产加密密钥做 `blake3` 承诺，绑定 VSS/VDD 证明。
- `secretSharingKey`: 买家侧交易密钥，脚本中由固定种子和 `originalAssetId` 派生。

