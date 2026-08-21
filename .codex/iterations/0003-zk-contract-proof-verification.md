# 迭代: ZK 程序合约证明验证

## 日期

2026-06-16

## 背景

上一轮 0002 已经完成 `drop-script` 协议审查和部分合约/脚本修复，但当前链上集成测试暂时继续使用已部署版本。本轮切换目标：不处理 ExchangeHub / ExchangeChannel / Oracle / subgraph 等外围流程，先把两个 zk 程序各自的“证明生成 -> Solidity verifier wrapper 验证”闭环跑通。

两个目标 zk 程序：

- VSS：`guest/vss/program`
- VDD：`guest/vdd/program-vdd-walrus-rslhve`

当前观察：

- `guest/vss/script` 已有 `evm`、`pn`、`vkey` 等 bin。
- `guest/vdd/script` 已有 `evm_walrus_rslhve`、`pn_walrus_rslhve`、`vkey_walrus_rslhve` 等 bin。
- `guest/vss/contracts/src/VSS.sol` 是 VSS verifier wrapper，包含 `verifyVSSProof` 和 `verifyVSS` binding hash 检查。
- `guest/vdd/contracts/src/VDD_RSLH.sol` 是 VDD RSLH-VE verifier wrapper，包含 `verifyVDDProof` 和 `verifyVDD` binding hash 检查。
- 现有 fixture 可能还不是完整真实证明：`guest/vss/contracts/src/fixtures/groth16-fixture.json` 和 `guest/vdd/contracts/src/fixtures/vdd-walrus-rslh-groth16-fixture.json` 中存在空 `proof` / `publicValues` 的情况，需要本轮确认并重新生成。

## 目标

- 让 VSS guest 生成 EVM 可验证 proof fixture。
- 让 VSS Solidity verifier wrapper 使用 fixture 验证通过。
- 让 VDD walrus_rslhve guest 生成 EVM 可验证 proof fixture。
- 让 VDD RSLH-VE Solidity verifier wrapper 使用 fixture 验证通过。
- 对每个 zk 程序明确：
  - guest public values
  - host fixture 字段
  - Solidity decoder
  - binding hash
  - SP1 verification key
  - proof bytes
  是否一致。

## 范围

允许修改，等待用户批准后才实施：

- `.codex/iterations/0003-zk-contract-proof-verification.md`
- `guest/vss/script/**`
- `guest/vss/contracts/**`
- `guest/vdd/script/**`
- `guest/vdd/contracts/**`
- 必要时更新 `.codex/docs/architecture.md`、`.codex/docs/context-supplement.md` 或新增 runbook 笔记

实施授权：

- 用户已批准按本计划开始实施。
- 用户确认本轮不需要设置 Chainlink 预言机。
- 用户确认本轮按合约证明验证目标推进；是否链上部署 wrapper 另行确认。
- 用户要求本轮串行推进：先闭合 VSS，再进入 VDD；不得并行推进两个 zk 程序。
- 用户要求开发阶段 zk 程序编译通过后默认不跑 execute，只有明确要求时才跑。
- 用户要求证明阶段使用 SP1 Prove Network，不使用本地 proving；证明失败后先诊断并停止，等待批准后才能再次提交，避免浪费已 approve 的 PROVE token。
- VSS 第一次 Prove Network 请求失败，浏览器显示 `Public values hash mismatch`，网络版本显示 `sp1-v6.1.0`；用户要求不使用 `guest/fibo3` 的现有代码，而是新建干净 probe guest，从标准 Fibonacci 逐步加入 VSS 写法排查。
- 用户批准最多使用 4 次 Prove Network 请求；每次请求必须有明确递进目的和结论。

默认不允许修改：

- `contracts/src/**` 主协议合约
- `drop-script/src/**`
- `subgraph/**`
- `drop-lib/**`，除非证明调试确认是库实现问题，并再次等待用户批准
- `.env` 文件内容不得写入 git

本轮不处理：

- ExchangeHub / ExchangeChannel 的 purchase、fulfill、settle、refund。
- Oracle / Chainlink Functions。
- Walrus publisher 上传下载。
- subgraph 索引。
- GUI。
- 端到端 `drop-script` 全流程。

保持不变的逻辑：

- VSS guest 语义不主动改。
- VDD walrus_rslhve guest 语义不主动改。
- RSLH-VE 密码学细节本轮不重新设计，只验证现有实现的证明与合约 wrapper 是否一致。

## 实施方法

### 阶段 0: 初始化和版本确认

1. 确认工作树干净。
2. 确认 SP1 crate 版本：
   - `sp1-sdk`
   - `sp1-zkvm`
   - `sp1-build`
   - `sp1-lib`
3. 确认 `guest/*/contracts/lib/sp1-contracts` 与 SP1 v6 proof/verifier 兼容。
4. 如果发现 Prover Network 当前不兼容本地版本，先记录并暂停，等待用户决定是否单独升级。

### 阶段 1: VSS 本地结构检查

检查对象：

- `guest/vss/program/src/main.rs`
- `guest/vss/script/src/bin/evm.rs`
- `guest/vss/contracts/src/VSS.sol`
- `guest/vss/contracts/test/VSS.sol`
- `guest/vss/contracts/src/fixtures/groth16-fixture.json`

需要确认：

- guest committed public values 与 Solidity `decodeVSS` 对齐。
- `evm.rs` 写出的 fixture 字段名、类型和 test 读取逻辑一致。
- `computeBindingHash(hOrigBlock, hKCommitment, cipherBlock)` 与 script / 主流程 binding hash 语义一致。
- fixture 中 `vkey`、`publicValues`、`proof` 均非空。

### 阶段 2: 干净 SP1 probe 基线

目的：

- 新建一个不依赖 `guest/fibo3` 的最小 Fibonacci probe guest/script。
- 使用当前项目 SP1 版本、同一把 `NETWORK_PRIVATE_KEY`、同一套 Prove Network 路径发起请求。
- 验证问题是 VSS 写法导致，还是当前 SP1 版本/请求环境/Network 兼容性导致。

执行规则：

- probe 文件只用于本轮排查，放在 `guest/probe-*` 命名空间。
- 每次证明前先低并发 `cargo build`，再运行已编译 binary，避免 `cargo run` 在证明阶段触发本地重编译。
- 不使用 `guest/fibo3` 代码作为基线。
- 每次 Prove Network 请求后，在本节记录请求编号、程序版本、目的、结果和结论。

请求预算：

1. 请求 1：干净 Fibonacci，确认 Prove Network 链路和 SP1 v6 写法是否正常。
2. 请求 2：在 probe 中加入 VSS 的 stdin 写入形态，但保留简单 public values，定位 stdin/request hash 影响。
3. 请求 3：在 probe 中加入 VSS 的 public values commit 形态，定位 public values hash mismatch。
4. 请求 4：修正后的 VSS 正式 fixture 请求。

当前版本记录：

- 本地 probe 使用 `sp1-zkvm = 6.2.4`、`sp1-sdk = 6.2.4`、`sp1-build = 6.2.4`。
- VSS 当前也使用 `sp1-zkvm = 6.2.4`、`sp1-sdk = 6.2.4`、`sp1-build = 6.2.4`。
- 用户在 Prove Network 页面看到的失败请求环境为 `sp1-v6.1.0`。
- `guest/probe-sp1/script` 已用低并发 `CARGO_BUILD_JOBS=1` 编译通过，构建耗时约 6m31s。

请求记录：

- 请求 0（VSS 失败请求，已发生）：VSS `target/debug/evm --system groth16` 使用 `NETWORK_PRIVATE_KEY="$SP1_PRIVATE_KEY"` 发起，Prove Network 页面显示 `Public values hash mismatch`。结论：问题发生在 request public values hash 与 execution oracle 结果不一致，需要从干净 SP1 写法开始排查，不继续重试 VSS。
- 请求 1（干净 Fibonacci probe）：`guest/probe-sp1` 使用同一 `SP1_PRIVATE_KEY -> NETWORK_PRIVATE_KEY` 临时映射、`sp1-sdk/sp1-zkvm/sp1-build = 6.2.4`、Groth16、Mainnet Prove Network。结果：成功生成 proof 并写入 `guest/probe-sp1/script/fixtures/probe-fib-groth16-fixture.json`。结论：私钥、PROVE allowance、Network 基础链路、最小 SP1 v6 写法均可用；VSS 的 `Public values hash mismatch` 更可能来自 VSS guest/script 写法或 public values 提交方式，而不是账户或网络整体不可用。
- 请求 2（VSS stdin 形态 + 简单 public values）：`guest/probe-vss-shape` 使用 VSS 的输入形态：`u8 length`、`Vec<u8>`、多个 `[u8; 32]`、多个 `[u8; 12]`，guest 内计算 `blake3::hash` 并提交简单 public values。结果：本地返回 `unfulfillable`，用户在浏览器看到 `Public values hash mismatch`。结论：问题在引入 VSS 输入/哈希相关写法后复现，但此请求同时包含 stdin 形态和 `blake3` 两个变量，不能单独归因。
- 请求 3（最小 blake3 probe）：`guest/probe-blake3` 只读取一个 `u32`，计算 `blake3::hash(&n.to_le_bytes())`，commit `n` 和 digest。结果：本地返回 `unfulfillable`。结论：若浏览器同样显示 `Public values hash mismatch`，则问题可高度怀疑为当前 `sp1-zkvm = 6.2.4` 的 `blake3` feature / blake3 相关执行在 Prove Network 当前 execution oracle 上不兼容；下一步用无 Fibonacci、无 blake3 的纯算术 probe 做对照。
- 请求 4（纯算术 probe）：`guest/probe-arith` 只读取两个 `u32`，执行 `wrapping_add`、`wrapping_mul`、`rotate`、`xor`，commit 输入和结果；无 Fibonacci、无 `blake3`。结果：成功生成 proof 并写入 `guest/probe-arith/script/fixtures/probe-arith-groth16-fixture.json`。结论：普通算术和普通 public values commit 在同一账户、同一 SP1 6.2.4 SDK、同一 Prove Network 上工作正常；失败范围进一步收敛到 `blake3` 相关路径。
- 请求 5（业务 BLAKE3 + SHA-256 public values digest）：`guest/probe-blake3-sha-pv` 保持和请求 3 相同的业务逻辑 `blake3::hash(&n.to_le_bytes())`，唯一关键差异是 `sp1-zkvm = "6.2.4"`，不启用 `features = ["blake3"]`。结果：成功生成 proof 并写入 `guest/probe-blake3-sha-pv/script/fixtures/probe-blake3-sha-pv-groth16-fixture.json`。结论：BLAKE3 业务运算本身可用；`Public values hash mismatch` 的根因是 `sp1-zkvm` 的 `blake3` feature 会把 zkVM 全局 public values digest 从默认 SHA-256 改为 BLAKE3，而当前 Prove Network 页面显示的 execution oracle `sp1-v6.1.0` 与该模式不兼容。
- 请求 6（VSS stdin 形态 + 业务 BLAKE3 + SHA-256 public values digest）：`guest/probe-vss-shape-sha-pv` 恢复请求 2 的完整 VSS stdin 形态和业务 `blake3::hash`，但不启用 `sp1-zkvm` 的 `blake3` feature。结果：成功生成 proof 并写入 `guest/probe-vss-shape-sha-pv/script/fixtures/probe-vss-shape-sha-pv-groth16-fixture.json`。结论：VSS stdin 序列化、动态 `Vec<u8>`、固定数组读取和业务 BLAKE3 均可在 Prove Network 正常执行；正式 VSS 的必要修复是从 `guest/vss/program/Cargo.toml` 移除 `sp1-zkvm` 的 `features = ["blake3"]`，保留独立 `blake3` crate 继续完成业务哈希。

正式 VSS 修复候选：

```toml
# 修复前
sp1-zkvm = { version = "6.2.4", features = ["blake3"] }

# 修复后
sp1-zkvm = "6.2.4"
```

该修改不改变 VSS 业务逻辑和 public values 内容，只把 zkVM 内部 public values digest 恢复为 Prove Network 当前兼容的默认 SHA-256。

正式 VSS 实施结果：

- 已将 `guest/vss/program/Cargo.toml` 的 `sp1-zkvm` 从启用 `features = ["blake3"]` 改为默认 feature 配置；独立 `blake3 = "1.8.2"` 和 guest 业务逻辑未改。
- 已用 `CARGO_BUILD_JOBS=1` 重新构建 `vss-script` 的 `evm` binary，新 VSS ELF 构建成功。
- 正式 VSS Groth16 Prove Network 请求成功，fixture 已写入 `guest/vss/contracts/src/fixtures/groth16-fixture.json`。
- fixture 中新 program vkey、public values 和 proof 均非空。
- 使用官方 Arbitrum Sepolia Groth16 gateway `0x397A5f7f3dBd538f23DE225B51f532c34448dA9B` 直接执行 `eth_call verifyProof(...)` 成功返回 `0x`，证明链上 verifier 可接受该 proof。
- 本机 Forge 版本为 `0.2.0`（2024-01-15），fork 测试在本地 revm 执行标准预编译时错误返回 `NotActivated`；这不是链上 verifier 或 proof 错误。
- VSS Foundry 测试已改为 mock gateway，分别覆盖 public values 解码、正确 binding hash、错误 binding hash 和假 proof；4 个测试全部通过。

Prove Network 计费观察：

- 用户确认当前失败请求不扣 credit，因此后续可以继续使用单变量 probe 排查。
- 仍然要求每次请求有明确目的和递进结论，禁止无差别重复提交。

### 阶段 3: VSS proof fixture 生成

候选命令：

```sh
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p vss-program
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p vss-script --bin evm
CARGO_BUILD_JOBS=1 PROTOC=/tmp/protoc-25.3/bin/protoc cargo build -p vss-script --bin evm
NETWORK_PRIVATE_KEY="$SP1_PRIVATE_KEY" ./target/debug/evm --system groth16
```

执行规则：

- `cargo check` 通过后不自动运行 execute。
- Groth16 证明必须使用 SP1 Prove Network。
- 不使用 `cargo run` 发起证明；先构建 binary，再运行已构建 binary。
- 如果证明提交或生成报错，先诊断错误和是否存在重复扣费/重复提交风险，然后停止等待用户批准。

### 阶段 4: VSS Solidity wrapper 验证

候选命令：

```sh
forge test --root guest/vss/contracts --match-contract VSSGroth16Test -vvvv
```

验收点：

- `test_ValidVSSProof` 通过。
- `testRevert_InvalidVSSProof` 通过。
- 若 wrapper 里 `verifyVSS` binding hash 尚未覆盖，需要补测试：
  - 正确 binding hash 返回 true。
  - 错误 binding hash revert。

### 阶段 5: VDD RSLH-VE 本地结构检查

检查对象：

- `guest/vdd/program-vdd-walrus-rslhve/src/main.rs`
- `guest/vdd/script/src/bin/evm_walrus_rslhve.rs`
- `guest/vdd/contracts/src/VDD_RSLH.sol`
- `guest/vdd/contracts/test/VDD_RSLH.sol`
- `guest/vdd/contracts/src/fixtures/vdd-walrus-rslh-groth16-fixture.json`

需要确认：

- guest public values 固定为 96 字节：`c_origin || c_key || c_cipher`。
- Solidity `decodeRSLHVE` 正确解码为 `cOrigin`、`cKey`、`cCipher`。
- `computeBindingHash(abi.encode(cOrigin, cKey, cCipher))` 与主流程 `VDD.submitVDDProof` 的 binding hash 一致。
- `c_key == blake3(key)` 的约束已经由 guest 执行。
- fixture 中 `vkey`、`publicValues`、`proof` 均非空。

### 阶段 6: VDD RSLH-VE proof fixture 生成

候选命令：

```sh
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check --manifest-path guest/vdd/Cargo.toml -p program-vdd-walrus-rslhve
PROTOC=/tmp/protoc-25.3/bin/protoc cargo check -p vdd-script --bin evm_walrus_rslhve
CARGO_BUILD_JOBS=1 PROTOC=/tmp/protoc-25.3/bin/protoc cargo build -p vdd-script --bin evm_walrus_rslhve
NETWORK_PRIVATE_KEY="$SP1_PRIVATE_KEY" VDD_RSLHVE_DATA_SIZE=65536 ./target/debug/evm_walrus_rslhve --system groth16
```

执行规则：

- 必须等 VSS 的 fixture 和 Solidity wrapper 验证闭合后，才能进入 VDD。
- `cargo check` 通过后不自动运行 execute。
- Groth16 证明必须使用 SP1 Prove Network。
- 不使用 `cargo run` 发起证明；先低并发构建 binary，再运行已构建 binary。
- 如果证明提交或生成报错，先诊断错误和是否存在重复扣费/重复提交风险，然后停止等待用户批准。

正式 VDD 实施结果：

- `program-vdd-walrus-rslhve` 原本已使用 `sp1-zkvm = "6.2.4"`，没有启用会切换 public values digest 的 `blake3` feature。
- 已从 `evm_walrus_rslhve` 移除证明前的本地 `client.execute(...)`；改为 Prove Network proof 返回后校验 proof public values，符合全局“默认不 execute”规则。
- 已用 `CARGO_BUILD_JOBS=1` 构建 `evm_walrus_rslhve` binary。
- 使用 64 KiB 输入生成 Groth16 Prove Network proof 成功，fixture 已写入 `guest/vdd/contracts/src/fixtures/vdd-walrus-rslh-groth16-fixture.json`。
- 使用官方 Arbitrum Sepolia Groth16 gateway `0x397A5f7f3dBd538f23DE225B51f532c34448dA9B` 直接执行 `eth_call verifyProof(...)` 成功返回 `0x`。
- VDD Foundry 测试使用精确 calldata mock gateway，分别覆盖 public values 解码、正确 binding hash、错误 binding hash 和假 proof；4 个测试全部通过。

### 阶段 7: VDD RSLH-VE Solidity wrapper 验证

候选命令：

```sh
forge test --root guest/vdd/contracts --match-contract VDD_RSLHTest -vvvv
```

验收点：

- `test_ValidRSLHVEProof` 通过。
- `testRevert_InvalidRSLHVEProof` 通过。
- 如果 wrapper 里 `verifyVDD` binding hash 尚未覆盖，需要补测试：
  - 正确 binding hash 返回 true。
  - 错误 binding hash revert。

### 阶段 8: 可选链上验证

本轮默认不部署主协议合约，但可以单独验证 wrapper 合约是否能在链上或 fork 上调用 SP1 verifier。

可选项：

- 使用 fork 测试调用当前 SP1 Verifier Gateway。
- 部署 VSS verifier wrapper 和 VDD verifier wrapper 到 Arbitrum Sepolia。
- 直接调用 wrapper 的 `verifyVSS` / `verifyVDD`。

这一步需要用户明确批准，因为会使用 `contracts/.env` 或 guest contracts env 里的私钥并消耗测试币。

## 研究笔记

### 本轮最小闭环定义

本轮“跑通合约证明部分”不等于跑通 `drop-script` fulfill。

最小闭环是：

```text
guest input
  -> SP1 EVM proof
  -> fixture JSON
  -> Solidity wrapper decode publicValues
  -> SP1 verifier gateway verifyProof
  -> wrapper binding hash check
  -> Foundry test pass
```

### VSS 验证边界

VSS wrapper 的关键接口：

```solidity
function verifyVSS(bytes calldata proof, bytes calldata publicValues, bytes32 bindingHash) external returns (bool)
```

需要同时验证：

- SP1 proof 有效。
- `publicValues` 能被 `decodeVSS` 正确解码。
- `bindingHash == keccak256(abi.encode(hOrigBlock, hKCommitment, cipherBlock))`。

### VDD 验证边界

VDD RSLH-VE wrapper 的关键接口：

```solidity
function verifyVDD(bytes calldata proof, bytes calldata publicValues, bytes32 bindingHash) external returns (bool)
```

需要同时验证：

- SP1 proof 有效。
- `publicValues` 能被 `decodeRSLHVE` 正确解码。
- `bindingHash == keccak256(abi.encode(cOrigin, cKey, cCipher))`。

注意：主协议合约 `contracts/src/VDD.sol` 中的 binding hash 使用 `dataKeyCommitment`，而 VDD guest public value 中第二段为 `c_key`。当前设计里二者应为同一值：`c_key == dataKeyCommitment == blake3(key)`。

## 测试验收标准

本轮基础验收：

- [x] VSS guest 和 EVM script 编译通过。
- [x] VDD walrus_rslhve guest 和 EVM script 编译通过。
- [x] VSS Groth16 fixture 中 `vkey`、`publicValues`、`proof` 非空。
- [x] VDD RSLH-VE Groth16 fixture 中 `vkey`、`publicValues`、`proof` 非空。
- [x] VSS proof 通过官方 Arbitrum Sepolia SP1 gateway `eth_call`。
- [x] VDD proof 通过官方 Arbitrum Sepolia SP1 gateway `eth_call`。
- [x] `forge test --root guest/vss/contracts --match-contract VSSGroth16Test`：4 passed。
- [x] `forge test --root guest/vdd/contracts --match-contract VDD_RSLHTest`：4 passed。

增强验收：

- VSS wrapper 的 `verifyVSS` binding hash 正向/反向测试通过。
- VDD wrapper 的 `verifyVDD` binding hash 正向/反向测试通过。
- 如果执行链上验证，记录部署地址、交易哈希和 verifier gateway 地址。

允许跳过：

- 完整 `drop-script` 端到端流程。
- Oracle 和 settlement。
- subgraph。
- VDD 大文件证明；若本地或 Prover Network 太慢，可以使用小数据 fixture 先完成合约证明闭环。

## 人工处理清单

本轮人工确认：

- [x] 已允许使用 SP1 Prover Network 生成 Groth16 proof。
- [x] 已允许读取 `drop-script/.env` 的 `SP1_PRIVATE_KEY`，运行时临时映射为 `NETWORK_PRIVATE_KEY`。
- [x] 本轮不运行本地 proving。
- [x] 本轮不运行默认 execute。
- [x] 本轮不部署 wrapper，不使用部署私钥或消耗 Arbitrum Sepolia ETH。

## 经验总结

- `sp1-zkvm` crate feature 可能改变 zkVM 协议行为，不只是开放业务 API。`features = ["blake3"]` 会把 public values digest 从 SHA-256 切换为 BLAKE3；迁移 SP1 版本时必须检查 feature 的协议语义。
- Prove Network 报 `Public values hash mismatch` 时，应使用单变量 probe 区分账户、stdin、业务哈希、public values commit 和 zkVM feature，不应直接修改业务逻辑。
- 证明命令必须拆成“低并发 build”和“直接运行 binary”两步，避免 `cargo run` 在发请求前重新编译并占满本机资源。
- Network SDK v6 使用 `NETWORK_PRIVATE_KEY`；项目现有 `SP1_PRIVATE_KEY` 通过命令环境临时映射，不需要复制或修改 `.env`。
- 本机 Forge `0.2.0` 的 fork/revm 对当前预编译模拟不可靠。真实 proof 验证使用官方 RPC 的 `cast call`，wrapper 解码和 binding hash 使用本地 mock 单元测试，职责分开。
- SP1 proof 会绑定新 ELF 的 program vkey；任何 guest 构建语义变化后都必须重新生成 fixture，并更新部署 wrapper 使用的 vkey。
