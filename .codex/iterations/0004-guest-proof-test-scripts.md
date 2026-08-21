# 迭代: Guest 证明测试脚本固化

## 日期

2026-06-22

## 背景

0003 已经完成 VSS 和 VDD walrus_rslhve 两个 zk 程序在 SP1 v6 / Prove Network 下的证明生成、官方 Arbitrum Sepolia verifier gateway 预检和本地 wrapper 测试。为了后续调试避免手工拼命令，本轮把 guest 测试流程固化为脚本。

## 目标

- 固化 VSS/VDD 的四阶段测试脚本：
  - 本地 execute。
  - 请求 Prove Network 生成 Groth16 fixture。
  - 本地合约 wrapper 测试。
  - 测试链官方 SP1 gateway 证明验证 preflight。
- 增加一个 Codex skill，解释这套脚本的用法和注意事项。

## 范围

允许修改：

- `guest/scripts/**`
- `.codex/skills/guest-proof-test/**`
- `.codex/iterations/0004-guest-proof-test-scripts.md`

保持不变：

- VSS/VDD guest 业务逻辑。
- VSS/VDD Solidity wrapper 逻辑。
- `.env` 文件内容。
- fixture 内容，除非用户显式运行 `prove` 阶段。

不处理：

- 主协议合约部署。
- subgraph。
- drop-script 端到端流程。
- 自动提交 Prove Network 请求；脚本只在用户显式调用 `prove` 或 `all` 时请求。

## 实施方法

- 新增统一入口 `guest/scripts/zk-proof-test.sh`。
- 使用参数形式：
  - `guest/scripts/zk-proof-test.sh vss execute`
  - `guest/scripts/zk-proof-test.sh vss prove`
  - `guest/scripts/zk-proof-test.sh vss local-contract`
  - `guest/scripts/zk-proof-test.sh vss preflight`
  - `guest/scripts/zk-proof-test.sh vss all`
  - `vdd` 同理。
- `prove` 先低并发 `cargo build`，再运行已编译 binary，避免 `cargo run` 在证明前触发重编译。
- `prove` 从 `drop-script/.env` 读取 `SP1_PRIVATE_KEY` 并临时映射为 `NETWORK_PRIVATE_KEY`，不打印私钥。
- `preflight` 用 `cast call` 只读调用官方 Arbitrum Sepolia SP1 Groth16 gateway。

## 研究笔记

- 官方 Arbitrum Sepolia Groth16 gateway 地址：`0x397A5f7f3dBd538f23DE225B51f532c34448dA9B`。
- 本地 Forge 版本较旧，不适合作为真实 SP1 verifier gateway fork 验证来源；真实 proof 验证用测试链 `eth_call` preflight，wrapper 解码和 binding hash 用本地 Foundry 测试覆盖。
- VDD 默认证明数据大小为 `65536`，脚本允许通过 `VDD_RSLHVE_DATA_SIZE` 覆盖。

## 测试验收标准

- [x] `guest/scripts/zk-proof-test.sh help` 能输出用法。
- [x] `bash -n guest/scripts/zk-proof-test.sh` 通过。
- [x] `guest/scripts/zk-proof-test.sh vss prove` 通过，已生成新的 Groth16 fixture。
- [x] `guest/scripts/zk-proof-test.sh vdd prove` 通过，已生成新的 Groth16 fixture。
- [x] `guest/scripts/zk-proof-test.sh vss preflight` 通过当前 fixture 验证，返回 `0x`。
- [x] `guest/scripts/zk-proof-test.sh vdd preflight` 通过当前 fixture 验证，返回 `0x`。
- [x] `guest/scripts/zk-proof-test.sh vss local-contract` 通过，4 passed。
- [x] `guest/scripts/zk-proof-test.sh vdd local-contract` 通过，4 passed。
- [x] skill 文档已说明四阶段命令、环境变量和风险边界。
- [x] skill 已同步到项目内 `.codex/skills/guest-proof-test/SKILL.md` 和全局 `/home/justin/.codex/skills/guest-proof-test/SKILL.md`。

未执行：

- `execute` 阶段未在本轮自动运行；脚本已固定命令，后续按需显式调用。

本次补充执行：

- 按用户要求顺序执行了 `vss prove -> vss local-contract -> vss preflight -> vdd prove -> vdd local-contract -> vdd preflight`。
- 两个 `prove` 阶段均成功请求 SP1 Prove Network 并刷新 fixture。
- 两个 `local-contract` 阶段均通过 4 个 Foundry wrapper 测试。
- 两个 `preflight` 阶段均通过 Arbitrum Sepolia 官方 SP1 Groth16 gateway `eth_call`，返回 `0x`。

## 经验总结

- guest proof 测试流程应拆分为四个明确阶段，避免把本地 execute、Prove Network、wrapper 单元测试和测试链 verifier 预检混在一起。
- `preflight` 是只读 `eth_call`，不需要加载或使用私钥；只有 `prove` 阶段需要读取 `SP1_PRIVATE_KEY` 并临时映射为 `NETWORK_PRIVATE_KEY`。
- `local-contract` 只验证 wrapper 解码和 binding hash，不代表真实链上 SP1 verifier；真实 verifier 预检必须跑 `preflight`。
- 证明阶段固定为先低并发 build，再直接运行 binary，避免 `cargo run` 在请求前触发重编译。
