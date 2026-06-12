# 维护建议

## 当前观察

- 根 README 信息不足，建议后续把 `docs/README.md` 链接回根 README。
- `drop-script` 是主要业务入口，但大量配置硬编码，包括链 RPC、合约地址、文件名、固定密钥和 Walrus endpoint。
- `contracts/deployed.md` 与 `drop-script/src/main.rs` 的地址存在不一致，需要确认哪个是当前可用部署。
- `drop-script` 的完整阶段逻辑和缺失集成项已整理到 `docs/drop-script.md`，后续修复应同步更新该文档。
- `guest/vss/script/Cargo.toml` 中的 patch 配置会被根 workspace 忽略。
- `app/gui` 仍是 Tauri 默认示例，没有接入业务 SDK。
- `guest` 子项目 README 多为 SP1 模板说明，需要补充业务语义。
- `guest/vss/contracts/lib` 和 `contracts/lib` 下有大量第三方依赖，整理项目结构时应避免把这些 vendored 文件当作自有模块。

## 建议的下一步

1. 配置外置化
   - 新增统一 `config.example.toml` 或 `.env.example`。
   - 将链 ID、RPC、合约地址、Walrus endpoint、输入输出文件名移出代码。

2. 文档继续细化
   - 为 VSS public values 和 VDD public values 增加字节级 ABI layout。
   - 为合约状态机增加时序图和失败路径说明。
   - 为 Oracle response 的 `status` 含义建立单独文档。

3. 开发体验
   - 在根 README 添加最短启动路径。
   - 增加 `justfile` 或 `Makefile` 封装常用命令。
   - 明确 workspace 中哪些 guest 是生产路径，哪些只是实验或模板。

4. 测试与验证
   - 继续使用 `forge test` 覆盖合约状态机。
   - 为 `drop-lib` 的加密、CID、RSLH-VE 增加确定性测试向量。
   - 为 `storage` 增加 provider mock，避免所有测试依赖外部网络。
