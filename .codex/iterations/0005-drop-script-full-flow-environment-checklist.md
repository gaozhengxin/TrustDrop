# 迭代: Drop Script 全流程环境检查

## 日期

2026-06-23

## 目标

- 为下一轮 `drop-script` 全流程跑通建立设计文档。
- 设计一个环境设置 checklist，后续实现为脚本和配套 skill，用来检查 Arbitrum Sepolia 测试网、Walrus 主网、合约、账号、Oracle、subgraph、SP1 Prove Network、guest proof fixture 等组件是否满足端到端运行条件。
- checklist 完成后先由用户运行一遍；未准备好的部分必须自动提示用户需要做什么人工操作。
- 本文档完成后等待用户确认，再进入任何代码、脚本或配置实现。

## 范围

允许本轮设计覆盖：

- `drop-script` 全流程运行前的环境检查项。
- 合约部署和地址同步检查。
- Arbitrum Sepolia 账户、余额、allowance、RPC 和链上代码检查。
- Walrus 主网 publisher / aggregator / blob 可访问性检查。
- Hybrid OracleProxy / centralized Oracle Worker / CRE-compatible path 依赖检查。
- subgraph 构建、部署配置和索引状态检查。
- SP1 Prove Network 配置检查。
- VSS/VDD verifier、fixture、guest ELF / VK 一致性检查。
- 用户需要手动处理的事项提示格式。

后续经批准后允许新增：

- 一个项目脚本，例如 `drop-script/scripts/check-env.sh` 或 `scripts/check-drop-env.sh`。
- 一个 Codex skill，例如 `.codex/skills/drop-script-env-check/SKILL.md`，解释 checklist 的使用方式、输出含义和人工处理步骤。
- 必要的文档索引更新。

已获用户确认并实施：

- 新增 `drop-script/scripts/check-env.sh`。
- 新增 `.codex/skills/drop-script-env-check/SKILL.md`。
- 已同步 skill 到全局 `/home/justin/.codex/skills/drop-script-env-check/SKILL.md`。

本轮不允许直接实施：

- 不修改 `drop-script` 业务逻辑。
- 不修改合约、subgraph、guest 程序或 fixture。
- 不部署合约。
- 不部署 subgraph。
- 不发交易。
- 不请求 SP1 Prove Network 证明。
- 不启动或修改 `/home/justin/walrus/start.sh`。
- 不读取或输出 `.env` 中的私钥、deploy key、API key。

保持不变的逻辑：

- VSS/VDD guest 逻辑保持 0004 收口版本。
- VSS/VDD verifier wrapper 和 fixture 的证明验证流程保持 0004 已验证状态。
- `drop-script` 协议阶段顺序暂不改变。

## 背景

0004 已经把 guest proof 测试固化为四阶段脚本，并完成：

- VSS Prove Network Groth16 证明生成。
- VSS 本地 wrapper 测试。
- VSS Arbitrum Sepolia 官方 SP1 gateway preflight。
- VDD Prove Network Groth16 证明生成。
- VDD 本地 wrapper 测试。
- VDD Arbitrum Sepolia 官方 SP1 gateway preflight。

下一步要跑 `drop-script` 全流程。这个流程不只是 zk proof，还依赖：

- Arbitrum Sepolia 上的 ExchangeHub / ExchangeChannel / VSS / VDD / Oracle 合约。
- Seller / buyer / deployer / SP1 prover 等账号。
- Walrus 主网 publisher 可上传，Oracle 侧可查询同一 blob。
- Hybrid OracleProxy 当前使用中心化 Oracle Worker 分支，后续保留 CRE-compatible 分支。
- subgraph Studio 项目配置和 manifest 地址。
- `drop-script/.env`、`contracts/.env`、`subgraph/.env` 三套环境配置。

当前已知需要重点检查的情况：

- `contracts/deployed.md` 记录的最新 Exchange hub 是 `0x1C01E8E981909926Ed67B5eEfAbfDfeCAcC882a1`。
- `drop-script/src/main.rs` fallback `HUB_ADDRESS` 仍是 `0x2e506eF3F3cE222F276ddA64Df239CEF92683a78`。
- `subgraph/README.md` 也仍记录旧 ExchangeHub `0x2e506eF3F3cE222F276ddA64Df239CEF92683a78` 和 start block `256170177`。
- 因此 checklist 必须把“`.env`、代码 fallback、部署文档、broadcast、subgraph manifest 的地址是否一致”作为硬检查；不一致时必须提示用户选择以哪个部署为准。

## 实施方法

本迭代建议分两步推进。

第一步，设计确认：

- 完成本设计文档。
- 用户确认 checklist 需要覆盖的组件和实现形态。
- 用户确认哪些检查允许联网、哪些检查只读链上、哪些检查允许调用本地 Walrus。

第二步，经用户确认后实施：

- 新增环境检查脚本。
- 新增 skill 文档。
- 只做只读检查和本地静态检查，不默认发交易、不默认部署、不默认证明。
- checklist 运行结束输出三类结果：
  - `PASS`: 当前机器或链上状态已满足。
  - `WARN`: 不一定阻塞，但需要用户确认。
  - `ACTION_REQUIRED`: 阻塞全流程，需要用户手动处理或明确授权 Codex 处理。

推荐实现形态：

- 脚本优先使用 shell 作为总入口，调用 `cast`、`curl`、`cargo check`、`forge`、`pnpm` 等已有工具。
- 对需要解析 JSON 的部分，优先使用 `jq`；如果环境没有 `jq`，脚本应给出缺失提示而不是静默失败。
- 不在脚本里打印私钥或完整 `.env` 内容；只显示地址、余额、合约代码存在性、配置是否存在。
- 脚本默认只读；需要交易、部署、证明、上传测试 blob 的动作必须拆成显式参数，不能在默认 checklist 中自动执行。

建议命令形态：

```sh
drop-script/scripts/check-env.sh
drop-script/scripts/check-env.sh --strict
drop-script/scripts/check-env.sh --json
drop-script/scripts/check-env.sh --section contracts
drop-script/scripts/check-env.sh --section oracle
drop-script/scripts/check-env.sh --section walrus
```

已实现命令：

```sh
drop-script/scripts/check-env.sh [--strict] [--json] [--section SECTION]
```

已实现 section：

- `tools`
- `env`
- `accounts`
- `contracts`
- `sp1`
- `walrus`
- `oracle`
- `subgraph`
- `drop-script`
- `manual`
- `all`

如果用户更希望放在仓库级别，也可改为：

```sh
scripts/check-drop-env.sh
```

## Checklist 设计

### 1. 基础仓库和工具

检查项：

- `git status --short`，提示当前是否有未提交改动。
- 当前路径是否为项目根目录。
- `cargo`、`forge`、`cast`、`pnpm`、`node`、`curl`、`jq` 是否可用。
- `/tmp/protoc-25.3/bin/protoc` 是否存在；如果不存在，提示用户安装或配置 `PROTOC`。
- `rust-toolchain.toml` 可用，Rust toolchain 能进入项目。

失败提示：

- 缺工具：提示安装或提供路径。
- 有未提交改动：提示用户确认是否允许在当前工作区继续集成测试。

### 2. 环境文件存在性

检查项：

- `drop-script/.env` 是否存在。
- `contracts/.env` 是否存在。
- `subgraph/.env` 是否存在。
- 不打印 secret，只检查必要变量是否存在。

`drop-script/.env` 需要检查：

- `SELLER_KEY`
- `BUYER_KEY`
- `SP1_PRIVATE_KEY`
- `ARBITRUM_SEPOLIA_RPC` 或 `ARBITRUM_SEPOLIA_RPC_URL`
- `WALRUS_LOCAL_ENDPOINT`
- `HUB_ADDRESS`
- `VSS_VERIFIER_ADDRESS`
- `VDD_VERIFIER_ADDRESS`
- `DROP_ORACLE_TIMEOUT_SECS` 可选

`contracts/.env` 需要检查：

- Arbitrum Sepolia RPC。
- 部署私钥。
- centralized Oracle Worker signer 配置。
- CRE forwarder 可选覆盖配置；默认使用 Arbitrum Sepolia 已知 forwarder。

`subgraph/.env` 需要检查：

- `SUBGRAPH_SLUG`
- `DEPLOY_KEY`

失败提示：

- 缺私钥：提示用户补 `.env`，不要求用户在聊天里发私钥。
- 缺 RPC：提示用户补 Arbitrum Sepolia RPC。
- 缺 subgraph deploy key：提示用户补 `subgraph/.env`。

### 3. 账号和余额

检查项：

- 从 `SELLER_KEY`、`BUYER_KEY`、`SP1_PRIVATE_KEY` 派生地址，但不打印私钥。
- Arbitrum Sepolia 链 ID 是否为 `421614`。
- Seller ETH 余额是否大于最低阈值。
- Buyer ETH 余额是否大于最低阈值。
- SP1 prover 地址 ETH 余额是否大于最低阈值。
- 如能只读查询 PROVE token，则检查 SP1 prover PROVE balance / allowance；如果地址或 ABI 不确定，输出 `WARN`，提示用户人工确认。

建议最低阈值先保守设置：

- Seller: `0.01 ETH`
- Buyer: `0.01 ETH`
- SP1 prover: `0.005 ETH`

失败提示：

- ETH 不足：提示用户给对应地址充值 Arbitrum Sepolia ETH。
- PROVE 不确定或不足：提示用户确认 Prove Network dashboard / allowance。

### 4. 合约地址和部署一致性

检查项：

- `drop-script/.env` 的 `HUB_ADDRESS`、`VSS_VERIFIER_ADDRESS`、`VDD_VERIFIER_ADDRESS` 是否存在且格式正确。
- 对这些地址执行 `cast code`，确认链上有代码。
- `contracts/deployed.md` 中最新地址与 `drop-script/.env` 是否一致。
- `contracts/broadcast/DeployMain.s.sol/421614/run-latest.json` 中部署地址与 `contracts/deployed.md` 是否一致。
- `subgraph/subgraph.yaml` 中 ExchangeHub 地址与当前选定 Hub 是否一致。
- `subgraph` startBlock 是否小于等于 Hub 部署区块。
- `drop-script/src/main.rs` fallback 地址是否与 `.env` 一致；不一致不阻塞，但输出 `WARN`，因为运行时应以 `.env` 为准。

链上只读检查：

- Hub code 非空。
- VSS verifier code 非空。
- VDD verifier code 非空。
- OracleProxy code 非空。
- Exchange logic code 非空。

需要进一步确认的合约关系：

- Hub 当前 channel implementation 地址是否为最新 Exchange logic。
- Hub 或 channel 当前配置的 VSS/VDD verifier 是否与 `.env` 一致。
- Hub 当前配置的 OracleProxy 是否为当前 OracleProxy。
- OracleProxy controller 是否为当前 Hub。
- OracleProxy defaultMode 是否为 centralized。
- OracleProxy CRE forwarder 是否为 Arbitrum Sepolia 预期地址。
- OracleProxy centralizedOracleSigner 是否已配置；Worker 私钥准备前该项允许作为 `ACTION_REQUIRED` 保留。

失败提示：

- 地址不一致：提示用户选择“沿用当前部署”或“重新部署并同步 env/subgraph”。
- 链上无代码：阻塞全流程，提示检查部署地址或重新部署。
- subgraph 地址不一致：提示更新 manifest 并重新部署 subgraph。

### 5. SP1 / guest proof 集成状态

检查项：

- `guest/scripts/zk-proof-test.sh` 是否存在且可执行。
- VSS fixture 是否存在。
- VDD fixture 是否存在。
- 可选只读 preflight：
  - `guest/scripts/zk-proof-test.sh vss preflight`
  - `guest/scripts/zk-proof-test.sh vdd preflight`
- `drop-script` include 的 VSS ELF 文件是否存在。
- `drop-script` include 的 VDD ELF 文件是否存在。
- 当前 verifier 地址是否对应当前 guest VK。若无法自动从合约读出 VK，则输出 `WARN` 并提示依赖 0004 证明记录或重新跑 verifier 部署检查。

默认不执行：

- 不跑 `execute`。
- 不跑 `prove`。
- 不重新生成 fixture。

失败提示：

- fixture 缺失：提示先运行 0004 guest proof workflow 的 prove 阶段。
- preflight 失败：阻塞全流程，提示先修复 guest/verifier/fixture 一致性。
- ELF 缺失：提示重新编译 guest。

### 6. Walrus 主网 publisher / aggregator

检查项：

- `/home/justin/walrus/start.sh` 是否存在。
- `WALRUS_LOCAL_ENDPOINT` 是否可连。
- root endpoint 返回 2xx 或 404 可视为服务在线。
- 检查 publisher 是否暴露上传相关 endpoint 的健康信息；如果接口不稳定，先只做连接性检查。
- 明确当前是 Walrus 主网，而不是测试网或本地 mock。若无法自动判断网络，输出 `ACTION_REQUIRED`，要求用户确认。

默认不执行：

- 不默认上传测试 blob。
- 不默认消耗 Walrus 存储额度。
- 不修改 Walrus 配置或私钥。

可选增强检查，经用户批准后再实现：

- 上传一个极小测试 blob。
- 从 aggregator 下载确认。
- 检查返回 blob id 是否能被 Oracle 查询端识别。

失败提示：

- 本地 endpoint 不通：提示用户运行 `/home/justin/walrus/start.sh`。
- 网络不确定：提示用户确认 Walrus mainnet context、Sui wallet、余额或存储配额。

### 7. Oracle / centralized Worker / CRE-compatible path

检查项：

- OracleProxy 地址有代码。
- OracleProxy controller 是否指向当前 ExchangeHub。
- OracleProxy centralizedOracleSigner 是否已配置。
- OracleProxy CRE forwarder 是否为 Arbitrum Sepolia 预期地址。
- OracleProxy defaultMode 是否为 centralized。
- Worker 是否已部署并配置 signer key。
- Worker signer 是否有足够 Arbitrum Sepolia ETH。
- Worker status 页面是否 ready，且不暴露具体余额或 secret。

默认不执行：

- 不触发 Worker report 交易。
- 不调用 setter。
- 不更新 Worker 私钥或配置。

可能需要人工处理：

- 用户准备并配置 centralized Oracle Worker 专用私钥。
- 用户确认 Worker signer 有 Arbitrum Sepolia ETH。
- 用户调用或授权调用 `OracleProxy.setCentralizedOracleSigner(<worker signer address>)`。
- 用户确认 Worker status 页面 ready，且不暴露具体余额或 secret。
- 用户配置或确认 Walrus API key / Blockberry API key。

当前已知风险：

- `cCipher` 是 32 字节 Walrus BlobId 原始 bytes。
- Oracle 侧是否把 bytes 正确编码为 Walrus API 可识别的 string 仍是后续调试重点。
- 如果 checklist 无法只读确认编码路径，必须输出 `WARN`，并在全流程跑到 fulfill/settle 时重点观察 Oracle 事件。

### 8. subgraph

检查项：

- `subgraph/.env` 存在且包含 `SUBGRAPH_SLUG`、`DEPLOY_KEY`。
- `subgraph/subgraph.yaml` 的 ExchangeHub 地址与当前选定 Hub 一致。
- startBlock 不晚于 Hub 部署区块。
- `pnpm --dir subgraph codegen` 可运行。
- `pnpm --dir subgraph build` 可运行。
- 如果网络允许，只读查询 Studio endpoint 或部署状态。

默认不执行：

- 不部署 subgraph。
- 不修改 Studio slug。

失败提示：

- manifest 地址旧：提示更新 `subgraph.yaml` 并重新 codegen/build/deploy。
- build 失败：提示先修复 schema/mapping/ABI。
- deploy key 缺失：提示用户补 `subgraph/.env`。

### 9. drop-script 编译和资产

检查项：

- `drop-script/KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile.mp4` 是否存在。
- `PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p drop-script` 是否通过。
- `drop-script` 链接的 ABI crate 是否与当前合约 ABI 同步。
- 当前 `.env` 地址是否覆盖了代码 fallback。

默认不执行：

- 不运行 `cargo run -p drop-script`。
- 不发交易。
- 不证明。
- 不上传 Walrus。

失败提示：

- 资产缺失：提示用户放入 `drop-script/KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile.mp4`。
- 编译失败：阻塞全流程，先修代码或 ABI。

## 用户人工处理清单

checklist 每次运行前或运行后都应输出以下人工处理项，并标明 `已检测通过`、`需要用户确认` 或 `需要用户操作`：

- [ ] 是否确认本轮使用 Arbitrum Sepolia 测试网，chain id `421614`？
- [ ] 是否确认 Walrus 使用主网 publisher，而不是测试网或 mock？
- [ ] 是否确认 `/home/justin/walrus/start.sh` 已启动并使用正确私钥？
- [ ] 是否确认 Walrus/Sui 钱包有足够余额或存储配额？
- [ ] 是否确认 `drop-script/.env` 中 seller、buyer、SP1 私钥可用于本轮测试？
- [ ] 是否确认 seller 和 buyer 地址有 Arbitrum Sepolia ETH？
- [ ] 是否确认 SP1 Prove Network key 有可用额度和 allowance？
- [ ] 是否确认当前 Hub / VSS / VDD / Oracle 合约地址以 `drop-script/.env` 为准？
- [ ] 如果地址不一致，是否选择重新部署合约并同步 `drop-script/.env`、`contracts/deployed.md`、`subgraph/subgraph.yaml`？
- [ ] 是否确认 centralized Oracle Worker 已部署并配置专用私钥？
- [ ] 是否确认 `OracleProxy.centralizedOracleSigner()` 已设置为 Worker signer？
- [ ] 是否确认 Worker signer 有 Arbitrum Sepolia ETH？
- [ ] 是否确认 Worker status 页面 ready，且不显示具体余额或 secret？
- [ ] 是否确认 CRE forwarder 保持 Arbitrum Sepolia 兼容配置？
- [ ] 是否确认 subgraph Studio slug 和 deploy key 可用？
- [ ] 是否允许后续在单独批准后运行 `drop-script` 全流程并发交易？

## 研究笔记

- 当前 `drop-script/src/config_check.rs` 已有基础检查：关键私钥存在性、Walrus endpoint、seller/buyer/SP1 ETH 余额、Hub/VSS/VDD 链上代码。
- 现有检查还不够覆盖全流程：缺少地址同步、Oracle Worker 配置、Walrus 主网确认、subgraph manifest、guest fixture / verifier preflight、资产文件和 ABI 同步检查。
- `drop-script` 当前 RPC env 使用 `ARBITRUM_SEPOLIA_RPC`；其他项目脚本可能使用 `ARBITRUM_SEPOLIA_RPC_URL`。checklist 应兼容读取两者，并提示统一。
- `drop-script` 当前 fallback Hub 与 `contracts/deployed.md` 最新 Hub 不一致，必须在跑全流程前确认 `.env` 覆盖值。
- `subgraph/subgraph.yaml` 当前指向最新 Hub `0x1C01E8E981909926Ed67B5eEfAbfDfeCAcC882a1`，start block 是 `280261185`。
- `subgraph/README.md` 仍指向旧 Hub；如果本轮以最新部署为准，README 需要后续更新。
- 0004 已证明 VSS/VDD guest proof 与官方 SP1 gateway 兼容，本轮 checklist 不应默认重复证明。
- 实施后局部运行 `tools` section 发现 `/tmp/protoc-25.3/bin/protoc` 当前不存在；这是后续完整 checklist 和 `drop-script` 编译前需要处理的 `ACTION_REQUIRED`。
- 实施后局部运行 `env` section 发现 `contracts/.env` 没有 Arbitrum Sepolia RPC 变量；如果后续部署或合约脚本依赖 `contracts/.env`，需要补齐或明确由外部环境提供。
- 首次完整 checklist 运行后发现脚本对 `contracts/deployed.md` 和 `subgraph/subgraph.yaml` 的地址解析依赖 `awk` 的 `{40}` 正则量词，当前环境下解析失败，导致合约/Oracle/subgraph 多个 false negative；已改为 `grep -Eo '0x[0-9a-fA-F]{40}'` 提取地址。
- 0007 合约迁移后重跑 `contracts` section：`PASS=20 WARN=0 ACTION_REQUIRED=1`，唯一操作项是 Worker signer 尚未配置。
- 0007 合约迁移后重跑 `oracle` section：`PASS=4 WARN=1 ACTION_REQUIRED=3 INFO=1`，操作项均为 Worker 未部署、Worker signer 未配置、Worker signer gas 余额确认。
- 修复后重跑 `subgraph` section：`PASS=8 WARN=1 ACTION_REQUIRED=0`，`codegen` 和 `build` 均通过。
- 修复后完整 checklist 结果：`PASS=74 WARN=7 ACTION_REQUIRED=8 INFO=15`。
- `drop-script/KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile.mp4` 已替换为 NASA 官方视频资产：
  - 标题：Apollo 11 25th Anniversary "B" Roll Footage。
  - NASA ID：`KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560`。
  - 下载 URL：`https://images-assets.nasa.gov/video/KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560/KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile.mp4`。
  - HEAD `content-length`: `102105235` bytes。
  - 本地大小：约 `98M`。
  - SHA256：`86da7fef254a6d88747cd88072dfdacd80a9f6b053b1750e9213ccf17da6fcef`。
  - 许可说明：NASA 官方媒体使用指南说明 NASA 内容通常在美国不受版权保护，可用于教育或信息用途；使用时不能暗示 NASA 背书，并应遵守 NASA 媒体使用指南。

## 测试验收标准

设计文档验收：

- [x] 文档明确本轮只设计，不实施。
- [x] 文档列出合约、账号、Oracle、Walrus、subgraph、SP1、guest proof、资产和工具链检查项。
- [x] 文档区分 Arbitrum Sepolia 测试网和 Walrus 主网。
- [x] 文档列出用户必须手动处理或确认的事项。
- [x] 文档说明未准备好时脚本应如何提示。

后续实施验收，需用户确认后执行：

- [x] 新增 checklist 脚本，默认只读、不发交易、不证明、不部署、不上传。
- [x] 新增 checklist skill，说明脚本用法和输出含义。
- [x] checklist 能在缺失配置时给出明确 `ACTION_REQUIRED`。
- [x] checklist 能识别地址不一致、链上无代码、Walrus 不通、subgraph manifest 旧地址、Oracle Worker 配置待确认等问题。
- [x] `bash -n drop-script/scripts/check-env.sh` 通过。
- [x] `drop-script/scripts/check-env.sh --section manual` 通过。
- [x] `drop-script/scripts/check-env.sh --section manual --json` 通过。
- [x] `drop-script/scripts/check-env.sh --section tools` 能输出 `PASS/WARN/ACTION_REQUIRED`，当前提示 `protoc` 缺失。
- [x] `drop-script/scripts/check-env.sh --section env` 能只报告变量存在性，不输出 secret，当前提示 `contracts/.env` 缺 RPC。
- [x] `drop-script/KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile.mp4` 已替换为 NASA 官方公共媒体素材，大小小于 400MB，并记录来源和 SHA256。
- [x] 已运行完整 checklist，并修正地址解析 false negative。
- [ ] 用户根据 checklist 输出补齐人工配置。

当前 checklist 阻塞项：

- [ ] 安装或恢复 `/tmp/protoc-25.3/bin/protoc`，或运行前设置 `PROTOC`。
- [ ] 在 `contracts/.env` 增加 Arbitrum Sepolia RPC 变量，或明确部署/合约脚本使用外部环境 RPC。
- [ ] 启动 Walrus publisher：`/home/justin/walrus/start.sh`，并确认 `http://localhost:31415` 可访问。
- [ ] 用户确认 Walrus publisher 是主网，并且钱包余额/存储额度可用。
- [ ] 用户准备 Worker 专用私钥并配置 centralized Oracle Worker。
- [ ] 调用 `OracleProxy.setCentralizedOracleSigner(<worker signer address>)`。
- [ ] 用户确认 Worker signer 有足够 Arbitrum Sepolia ETH。
- [ ] 用户确认 Worker status 页面 ready，且不显示具体余额或 secret。

## 经验总结

- 全流程调试前必须先做环境闭合检查，否则问题会混在交易、证明、Oracle、Walrus 和 subgraph 多个层面里，很难定位。
- 默认 checklist 应保持只读，所有会消耗资金、额度或外部服务状态的动作都要拆成显式批准步骤。
- 对本项目而言，地址一致性是首要门槛：`drop-script/.env`、部署文档、broadcast、subgraph manifest 和代码 fallback 必须明确谁是事实来源。
- Walrus 主网与 Arbitrum Sepolia 是跨网络组合，必须在 checklist 里单独要求用户确认 Walrus 网络、publisher 私钥和存储额度。
