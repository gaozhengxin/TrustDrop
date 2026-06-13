# Codex 项目知识与进度管理

本目录用于沉淀 Codex 在本项目中的长期上下文、标准流程、迭代记录和可复用操作方法。目标是让后续任何一次 Codex 会话都能快速恢复项目状态，按统一方法推进，而不是重新从零分析。

## 目录结构

- `docs/`: 从原项目 `docs/` 迁入的项目知识库，包含架构、模块、合约、运行方式和 `drop-script` 集成逻辑。
- `README.md`: 本文件，维护 Codex 工作方式、项目推进标准流程、迭代模板和标准化命令。

建议后续新增：

- `iterations/`: 每次迭代的过程记录和结论。
- `runbooks/`: 可重复执行的操作手册，例如部署、证明生成、合约验证。
- `decisions/`: 关键技术决策记录。

## 当前知识入口

- [项目总览](./docs/project-overview.md)
- [架构与协议流程](./docs/architecture.md)
- [模块说明](./docs/modules.md)
- [运行与环境](./docs/operations.md)
- [合约与部署](./docs/contracts.md)
- [Drop Script 端到端流程](./docs/drop-script.md)
- [维护建议](./docs/maintenance.md)

## 项目推进标准流程

每次迭代按以下顺序推进。

### 0. 开发宪章

任何涉及业务代码、合约、subgraph、配置、依赖版本、部署脚本或测试环境的工作，都必须遵守以下门禁：

1. 文档先行。先建立或更新 `.codex/iterations/` 下的本次迭代文档。
2. 迭代文档只记录本次迭代的背景、目标、范围、实施方法、研究笔记、测试验收标准和经验总结。
3. 如果研究中发现需要代码改动，只能在文档中描述候选改动和理由，不能直接实施。
4. 必须等待用户明确批准后，才能修改业务代码、合约、subgraph、配置或依赖。
5. 用户批准实施后，代码 diff 必须和迭代文档逐项对照；不匹配时停止实施，更新文档并等待再次决策。
6. 未经批准已经产生的 diff 只能作为复盘材料记录，不得默认视为已接受实现。
7. `.env`、私钥、API key、deploy key、证明账户凭证不得写入 git 或项目知识库。

### 1. 明确目标

需要先在迭代文档中写清楚：

- 本轮要交付什么行为或文档。
- 哪些模块允许修改。
- 哪些逻辑必须保持不变。
- 本轮不处理哪些问题。
- 是否需要等待用户批准后才能实施。

输出要求：

- 目标可验证。
- 边界清楚。
- 如果依赖外部服务，列出服务和环境变量。

### 2. 读取现状

开始修改前必须收集当前状态：

```sh
git status --short
find .codex/docs -maxdepth 2 -type f -print
cargo metadata --no-deps --format-version 1
```

对代码任务，应优先读取：

- `Cargo.toml` 和相关 crate 的 `Cargo.toml`。
- 被修改模块的主入口文件。
- 已有测试和脚本。
- 相关文档。

原则：

- 不覆盖用户已有改动。
- 不做无关重构。
- 不把临时调试逻辑留在主路径。

### 3. 制定实施方法

实施方法应回答：

- 要改哪些文件。
- 为什么必须改。
- 会影响哪些构建、测试、合约或脚本。
- 如何验证。

如果有外部限制，例如 SP1 proving 太慢、网络不可用、系统依赖缺失，应提前记录。

### 4. 等待用户决策

在用户批准前，只能继续补充研究笔记和验收标准，不能执行实现。

用户决策应明确：

- 批准实施的范围。
- 不允许修改的文件或模块。
- 是否允许安装依赖、联网查询、部署、运行外部服务。
- 是否允许提交 commit。

### 5. 执行修改

修改原则：

- 优先沿用现有项目结构。
- Rust 代码使用 `cargo fmt`。
- 文档与代码同时更新。
- 对协议逻辑，必须说明 public values、binding hash、合约 ABI 和 host 输入是否一致。

### 6. 验证

验证分层执行。

基础验证：

```sh
cargo check -p drop-lib
cargo test -p drop-lib
```

SP1 guest 和 script 验证：

```sh
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p vss-program
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p vss-script --bin vss
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p program-vdd-walrus-rslhve
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p vdd-script --bin main_walrus_rslhve
```

端到端脚本验证：

```sh
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p drop-script
```

可选 execute：

```sh
PROTOC=/tmp/protoc-25.3/bin/protoc cargo run -p vss-script --bin vss -- --execute
PROTOC=/tmp/protoc-25.3/bin/protoc VDD_RSLHVE_DATA_SIZE=65536 cargo run -p vdd-script --bin main_walrus_rslhve -- --execute
```

说明：

- VSS/VDD 完整 proving 可能很慢，本地验证优先 `cargo check` 和小数据 `execute`。
- `drop-script` 依赖 SP1 v6 runner，部分环境下需要在沙箱外运行 `cargo check`。
- 若 `/tmp/protoc-25.3/bin/protoc` 不存在，需要重新安装或改用系统 `protoc`。

### 7. 更新知识库

任何涉及以下内容的改动都要同步 `.codex/docs/`：

- 协议阶段变化。
- 合约 ABI 或 binding hash 变化。
- guest public values 变化。
- 环境变量变化。
- 构建或测试命令变化。
- 已知限制和后续补齐项变化。

### 8. 收尾

收尾输出应包含：

- 变更摘要。
- 验证命令和结果。
- 未能验证的项目及原因。
- 剩余风险。
- 下一步建议。

## 每次迭代文档模板

建议保存为 `.codex/iterations/YYYY-MM-DD-short-title.md`。

```md
# 迭代: <标题>

## 日期

YYYY-MM-DD

## 目标

- <本轮要完成的具体目标>
- <本轮等待用户决策的问题，如有>

## 范围

- 允许修改:
- 不允许修改:
- 保持不变的逻辑:

## 背景

- <为什么要做>
- <当前已知状态>

## 实施方法

- <计划如何研究或实施>
- <关键取舍>
- <风险点>

## 研究笔记

- <事实记录、命令摘要、版本观察、外部依赖观察>
- <未经批准的候选 diff 或已发生 diff 只能放在这里作为待审材料>

## 测试验收标准

- <必须通过的命令或链上检查>
- <允许跳过的验证及原因>
- <验收通过的判断标准>

## 经验总结

- <本轮结束后沉淀的方法、风险和下次注意事项>
```

## 标准化方法

### 构建 Rust workspace

优先按包构建，不直接全 workspace 构建 GUI 相关内容。全 workspace 可能因为 Tauri/GTK/WebKit 系统依赖失败。

推荐：

```sh
cargo check -p drop-lib
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p drop-script
```

谨慎使用：

```sh
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check --workspace
```

### 构建测试环境

基础环境：

- Rust toolchain 使用仓库 `rust-toolchain.toml`。
- SP1 toolchain 可构建 v6 guest。
- `protoc` 可用。
- Walrus daemon 可访问。
- Arbitrum Sepolia RPC 可访问。
- `.env` 包含 `SELLER_KEY`、`BUYER_KEY`、`SP1_PRIVATE_KEY`。

`protoc` 检查：

```sh
/tmp/protoc-25.3/bin/protoc --version
```

Walrus 检查：

```sh
curl -I http://localhost:31415
```

### 运行 drop-script 前置检查

运行前确认：

- `.codex/docs/drop-script.md` 中记录的限制已理解。
- 合约地址与 `contracts/deployed.md` 对齐。
- 链上 verifier 的 VK 与当前 guest ELF 对应。
- `Mo.mp4` 存在。
- Walrus 可上传下载。
- Oracle proxy 可正常回调。
- SP1 network 账户可提交 Groth16 proof。

### 证明相关方法

本地优先验证：

- `cargo check`
- 小数据 `execute`
- public values host 校验
- verifier 静态调用

网络 proving 只在以下条件满足时执行：

- guest ELF 已固定。
- verifier 合约已用对应 VK 部署。
- 输入数据规模可控。
- SP1 prover network 账户和余额确认可用。

### 文档维护方法

文档更新规则：

- `.codex/README.md` 维护流程和方法。
- `.codex/docs/` 维护项目事实。
- 迭代记录维护过程和结论。
- 不在文档里记录私钥、API key、未脱敏部署密钥。

链接规则：

- `.codex/docs/README.md` 作为项目知识索引。
- 本文件作为 Codex 工作索引。
- 新文档必须从其中一个入口可达。

## 当前重点风险

- `drop-script` 中 purchase/fulfill 的 ECIES ephemeral pubkey 目前在单次脚本内存中传递，跨进程执行需要链上事件或链下元数据持久化。
- `drop-script` 的合约地址仍硬编码，需要配置化并与部署文档统一。
- VDD guest 已约束 `c_key == blake3(key)`，后续如修改 key commitment 算法必须同时更新合约、host 和 guest。
- GUI 仍未接入业务流程。
- 全 workspace 构建可能受系统 GUI 依赖影响。
