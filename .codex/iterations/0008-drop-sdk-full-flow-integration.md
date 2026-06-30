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

## 2026-06-29 收口补记

本轮后续被切回 0008 收口：先暂停 `drop-cli`，清理本轮不该出现的临时 resume 代码，然后补齐合约、guest、Oracle Worker、subgraph 与 `drop-script` 主流程的结构性闭合检查。

### 已完成的收口动作

- 删除临时 `drop-script/src/bin/resume_drop_cli_sale.rs`，不保留隐藏 resume helper。
- 停止继续改 `drop-cli`，本轮只处理 0008 未闭合的 `drop-script` / 合约 / worker / subgraph 链路。
- VSS fixture 通过本地 wrapper 测试和 Arbitrum Sepolia SP1 gateway preflight。
- VDD walrus_rslhve fixture 通过本地 wrapper 测试和 Arbitrum Sepolia SP1 gateway preflight。
- 发现链上已部署 VSS/VDD verifier wrapper 的 VK 与当前 fixture 不一致，重新部署 verifier wrapper，并更新 Hub verifier 指针。
- 修复 `drop-script` VSS SP1 stdin 写法，使其与 guest 读取格式一致。
- 将 VDD proof 提前到 listing/key commitment 后提交；fulfill 阶段只处理 buyer 相关的 VSS。
- 修复 Walrus 刚上传后短时间不可下载的问题：VDD 证明下载 Walrus blob 时增加 backoff。
- 修复 Oracle Worker 上链 report 状态映射：
  - `/walrus/blob-status` 查询 API 保持 `0=active, 1=expired/not_found`。
  - `/oracle/fulfill` 上链 report 映射到合约协议：`2=Ensured, 1=Retrievable, 0=Fail`。
- 部署更新后的 Oracle Worker。
- 修复 `drop-script` recovery 事件读取：从 `fulfill_tx_hash` 的 receipt 解析 `DataKeyShared`，不再依赖宽泛历史 `eth_getLogs`。
- `drop-script` 编译通过。
- subgraph `codegen` / `build` 通过；本轮没有 Hub 地址、ABI 或 mapping 变更，不需要重新部署 subgraph。

### 本轮关键部署和交易

- 新 VSS verifier wrapper：`0x90933a2D8556Bf0785be48D95516238F8C788eBf`
- 新 VDD verifier wrapper：`0x23e85B3d3dCD4597a40CcDE987ac2BA5c7F3481D`
- Hub `setVSSVerifier` tx：`0x8b79929ea1bf3301a903fa2f921fdf39cae4bdf12c37a7846b7ef7ca6a5864c5`
- Hub `setVDDVerifier` tx：`0x1b016fe56efe80784ba93d8c9bc9c07d712fc391d234577de388d5d7f68a46dc`
- Oracle Worker deployed version：`414f6d6c-5af6-4b79-b080-4e1f480f7a01`
- 本次 live flow channel：`0x79138b586c69ac33eeb1869a989f0faacff7b349`
- Sale id：`0xa77219fd379011f8fd0bbded84567f3b826569056fcd786e75327f2d261bdde4`
- Walrus blob id：`ZzuLyCPR63KYwdTWBySUPedk5SdiJ0LZ4gj2QtgXvjk`
- VDD submit tx：`0x3ca8a1ba10127549d6496e3c19e5e604c552690b80d52b5f3146919dcce6899d`
- 初次 Worker report tx：`0xad024fc481a2e105d920c538e3c6f2e8e144548b4eaa25501da030234d04017a`
- 修复后手动 retrigger oracle tx：`0xa8d2f2c45ccd9a73152b164253c7673742875eb9bf328ae401543df2fc3a0334`
- 修复后 Worker report tx：`0x47f2ade854f9ce9e66f2a070dc6085d285079906ccffae1409e60f545eee70be`
- Fulfill tx：`0x080471f718a775dd942391d84061e5c899361616a6f4b0f51e4e2628d7b843f2`
- Settle tx：`0x0c37467e893dd70939968861c52da44fde7a18d400c0fbe1cb2f9a5bd5dcf3b7`

### 收口核验结果

- `vddVerified(cCipher) == true`
- `isPrivy(buyer) == true`
- `oracleSuccessUntil(cCipher) == 1786406400`
- Hub 已发出 `SettleEvent(channel, buyer, saleId, dataCommitment)`。
- `DataKeyShared` 已在 fulfill receipt 中发出；旧 recovery 失败是日志查询方式问题，不是 fulfill/VSS 失败。

### 每次 live 调试必须逐项检查

- 合约地址：
  - `drop-script/.env` 的 `HUB_ADDRESS`、`VSS_VERIFIER_ADDRESS`、`VDD_VERIFIER_ADDRESS`。
  - `contracts/deployed.md` 是否同步。
  - `subgraph/subgraph.yaml` Hub 地址和 startBlock 是否对应当前 Hub。
- verifier / guest 匹配：
  - 当前 VSS fixture VK 是否等于链上 VSS wrapper `VSSProgramVKey()`。
  - 当前 VDD fixture VK 是否等于链上 VDD wrapper `VDDProgramVKey()`。
  - VSS/VDD fixture 是否分别通过本地 wrapper test 和 SP1 gateway preflight。
- `drop-script` 编排：
  - VDD 是 sale/data 级别，应在 prepare/listing 后提前生成并提交；fulfill 阶段不应重复生成 VDD。
  - VSS 是 buyer 级别，应在 fulfill 阶段为当前 buyer 生成并提交。
  - recovery 必须从确定的 fulfill transaction receipt 解析 `DataKeyShared`。
- Walrus：
  - 上传后记录原始 Walrus blob id。
  - `cCipher` 必须能转换回同一个 Walrus blob id。
  - Worker `/walrus/blob-status` 对 blob id 和 cCipher 两种查询都应返回同一对象。
  - 新上传 blob 可能短时间不可下载，VDD proof 前需要 backoff。
- Oracle Worker：
  - `/status` 必须 `ok=true`，relayer 无 pending tx，relayer 与 OracleProxy signer 匹配。
  - 查询 API 状态编码与合约 report 状态编码不能混用。
  - active blob 上链 report 必须给合约 `status=2` 或 `status=1`，不能给 `0`。
  - 每次 `OracleRequested` 后要确认 Worker report tx 成功，并读取 `oracleSuccessUntil(cCipher)`。
- fulfill / settle：
  - `fulfill` receipt 必须含 `DataKeyShared`。
  - `isPrivy(buyer)` 必须为 true。
  - `vddVerified(cCipher)` 必须为 true。
  - `oracleSuccessUntil(cCipher)` 必须大于当前交易要求的可用窗口。
  - `settle` 后 Hub 必须发出 `SettleEvent`。
- subgraph：
  - 只要 Hub 地址、ABI、schema、mapping 未变，不需要重新部署。
  - 如任一项变化，必须先 `pnpm --dir subgraph codegen`、`pnpm --dir subgraph build`，再按批准部署 Studio 新版本。

### 经验总结

- 不能只看 Prove Network 或 wrapper preflight 通过；必须确认链上正在使用的 wrapper VK 与当前 guest fixture 一致。
- Oracle Worker 对外查询 API 可以用便于调试的状态码，但上链 report 必须严格遵守合约协议。
- VDD 和 VSS 生命周期不同：VDD 是数据/sale 级别，VSS 是 buyer/fulfill 级别。把二者都塞进 fulfill 会导致脚本难以恢复，也浪费证明请求。
- 端到端脚本的每一步都必须打印关键交易哈希，否则失败后无法可靠恢复和核验。
- 不能把“跑到某一步”描述成“全流程跑通”。本轮真正闭合的是 VDD submit、oracle fulfill、buyer purchase、VSS fulfill、settle；recovery 的旧失败已定位并修复为确定 receipt 解析。
