# 迭代: Drop SDK 拆分与全流程集成测试

## 日期

2026-06-25

## 背景

0007 已完成中心化 Oracle Worker 与合约适配的结构闭合：

- Cloudflare Oracle Worker 已部署并通过最小链上集成测试。
- 合约层已覆盖 `OracleProxy` 回调真实 Channel、更新 `oracleSuccessUntil`、继续 `settle` 的本地测试。
- `drop-script` 已加入 centralized Worker 触发逻辑。

但当前 `drop-script` 仍承担过多职责：环境读取、Walrus 上传下载、合约交互、SP1 证明请求、Oracle Worker 触发、事件解析和流程编排混在一个脚本中。下一步需要把可复用能力拆分到 `drop-sdk`，再用 SDK 驱动完整流程集成测试。

本轮还要补齐 0007 留下的 live full-flow 验证：Walrus 主网节点、subgraph、Oracle Worker、Arbitrum Sepolia 合约与脚本/SDK 必须在同一流程中闭合。

## 目标

- 将已有 `maenad-sdk` 包改名为 `drop-sdk`。
- 设计并实施 `drop-script` 到 `drop-sdk` 的能力拆分。
- 保留 `drop-script` 作为端到端 CLI / demo 编排层，减少业务细节堆积。
- 完成 Walrus 节点、subgraph、Oracle Worker、合约与 SDK/script 的全流程集成测试。
- 明确哪些步骤可自动化，哪些步骤需要用户手动确认或操作。
- 形成后续 `drop-sdk` 可继续产品化的接口边界。

## 范围

本轮预计覆盖：

- `drop-script` 现有流程梳理与模块拆分计划。
- `drop-sdk` crate/API 设计。
- Walrus 主网 publisher / aggregator 可用性检查。
- Subgraph 当前部署的 Studio deploy / publish 状态确认与补齐。
- Oracle Worker health/status/fulfill 与合约适配验证。
- Arbitrum Sepolia 合约地址、verifier、OracleProxy、Worker signer 的一致性检查。
- 完整 live full-flow 集成测试方案。

本轮暂不默认执行：

- 不默认提交 SP1 Prove Network 证明请求。
- 不默认运行完整 `drop-script` live flow。
- 不默认重新部署合约、subgraph 或 Worker。
- 不默认修改密码学协议细节。

任何会触发证明请求、链上交易、Walrus 上传、subgraph deploy/publish 或 Worker 重新部署的操作，都需要先在设计文档中列明，再等待用户确认。

## 当前实施记录

用户调整本轮优先级：仓库中已有 `sdk/` crate，原包名为 `maenad-sdk`。本轮先不新增大规模 SDK 模块，而是先完成命名统一、轻量结构整理和用户文档。

已实施：

- `sdk/Cargo.toml` package name 从 `maenad-sdk` 改为 `drop-sdk`。
- `drop-script/Cargo.toml` 依赖从 `maenad-sdk` 改为 `drop-sdk`。
- 根 `Cargo.toml` 依赖从 `maenad-sdk` 改为 `drop-sdk`。
- `drop-script/src/main.rs` import 从 `maenad_sdk::...` 改为 `drop_sdk::...`。
- 清理 `sdk/src/lib.rs` 中的占位 `add()` 示例函数。
- 对 `sdk/src/chacha8.rs`、`sdk/src/proof.rs`、`sdk/src/walrus.rs` 做轻量格式整理，不改变行为。
- 新增 `sdk/README.md`。
- 新增 `.codex/docs/drop-sdk.md`。

验证：

- `PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p drop-script` 已通过。
- `PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p drop-sdk` 已通过。

未实施：

- 未运行完整 `drop-script`。
- 未发 SP1 Prove Network 请求。
- 未做链上交易。
- 未部署 subgraph / Worker / 合约。

## 初始实施方法

1. 先做只读盘点：
   - `drop-script` 当前主流程和可拆分函数。
   - 是否已有 `drop-sdk` 包或可复用库边界。
   - 当前 Walrus、subgraph、Oracle Worker、合约部署记录。
2. 建立 SDK 拆分设计：
   - 合约客户端层。
   - Walrus 存储层。
   - ZK proof orchestration 层。
   - Oracle Worker client 层。
   - E2E workflow 层。
3. 建立全流程集成测试矩阵：
   - 本地只读检查。
   - 本地合约模拟。
   - 测试链 preflight。
   - live full-flow。
4. 用户确认设计后再实施代码拆分和外部服务操作。

## 研究笔记

- 0007 证明了 Worker/OracleProxy/Channel callback 的基础链路可用。
- 完整 live flow 的主要风险已经从 Oracle Worker 基础逻辑转移到：
  - `drop-script`/未来 SDK 的状态编排。
  - SP1 proof 请求与 verifier VK/合约地址一致性。
  - Walrus 上传结果与 Worker 查询路径的一致性。
  - subgraph 是否索引当前 Hub 和最新 startBlock。
  - 脚本失败后的可恢复性。

## 测试验收标准

初始验收标准，后续设计阶段可细化：

- `drop-sdk` 能承载从 `drop-script` 拆出的核心可复用能力。
- `drop-script` 仍能作为 thin CLI / demo 跑完整流程。
- Walrus 节点 readiness 有明确检查命令和结果。
- Oracle Worker `/status` 正常，且 fulfill 流程能被 SDK/script 触发。
- Subgraph 完成当前部署的 Studio deploy；是否 publish 需要单独决策。
- 完整 live full-flow 的每一步都有可观测输出和失败恢复说明。

## 经验总结

待本迭代推进后补充。
