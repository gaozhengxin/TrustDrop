# Maenad / TrustDrop 项目文档

本文档目录是在当前 `dev` 分支上按仓库现状整理的项目知识库。项目根 README 当前只有标题，因此这里把代码结构、协议流程、运行方式和合约关系集中放到 `.codex/docs/` 下维护。

## 文档索引

- [项目总览](./project-overview.md): 项目目标、技术栈、仓库结构和核心概念。
- [架构与协议流程](./architecture.md): 端到端交易流程、链上/链下组件职责、数据流。
- [模块说明](./modules.md): Rust workspace、SP1 guest、智能合约、GUI、存储模块逐项说明。
- [运行与环境](./operations.md): 本地开发、测试、存储服务、SP1 证明和环境变量。
- [合约与部署](./contracts.md): 合约职责、事件、Arbitrum Sepolia 已记录地址。
- [Drop Script 端到端流程](./drop-script.md): `drop-script` 的阶段逻辑、ZK/合约集成点和缺失项。
- [维护建议](./maintenance.md): 当前代码观察到的风险、文档后续维护项。

## 快速定位

- 端到端演示主流程: `drop-script/src/main.rs`
- 通用密码学与数据承诺: `drop-lib/src/`
- 存储抽象和 Walrus/Filecoin 客户端: `storage/src/`
- 链上交易通道与 VSS/VDD 合约: `contracts/src/`
- SP1 guest 程序: `guest/vss/program/`、`guest/vdd/program-vdd-walrus-rslhve/`
- Tauri GUI 雏形: `app/gui/`
