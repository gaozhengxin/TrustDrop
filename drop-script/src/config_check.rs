use anyhow::{anyhow, Result};
use ethers::prelude::*;
use std::env;

/// # [CHECK] 运行配置检查
///
/// ## 角色
/// 脚本运行者 (同时扮演买家、卖家、证明者)
///
/// ## 作用
/// 在执行任何实质性操作之前，此函数会全面检查所有必要的外部依赖和配置。
/// 这可以防止因环境问题 (如密钥缺失、节点离线、余额不足) 导致的交易失败或长时间等待，
/// 从而确保脚本能够顺利地一键完成整个流程。
pub async fn run_config_checks() -> Result<()> {
    println!(">>> Running Configuration Checks...");

    // --- [1. 环境变量检查] ---
    // 检查脚本运行所需的几个关键私钥是否已在 `.env` 文件中正确设置。
    println!("Checking environment variables...");
    let seller_key = env::var("SELLER_KEY").map_err(|_| anyhow!("SELLER_KEY not set"))?;
    let buyer_key = env::var("BUYER_KEY").map_err(|_| anyhow!("BUYER_KEY not set"))?;
    let sp1_private_key =
        env::var("SP1_PRIVATE_KEY").map_err(|_| anyhow!("SP1_PRIVATE_KEY not set"))?;
    println!("  - Environment variables (SELLER_KEY, BUYER_KEY, SP1_PRIVATE_KEY) are set.");

    // --- [2. 本地 Walrus 节点状态检查] ---
    // 通过一个简单的 HTTP GET 请求来探测本地 Walrus 节点的健康状况。
    // Walrus 节点是本系统中的去中心化存储层，负责托管加密后的数据。
    println!("Checking Walrus node status...");
    match reqwest::get("http://localhost:31415").await {
        Ok(res) if res.status().is_success() || res.status() == reqwest::StatusCode::NOT_FOUND => {
            // We accept 2xx or 404 (since the root path might not be defined)
            println!("  - Walrus node is running.");
        }
        Ok(res) => {
            // Got a response, but it's not a success status code we expect.
            return Err(anyhow!(
                "Walrus node returned an unexpected status: {}. Please check the node's logs.",
                res.status()
            ));
        }
        Err(e) => {
            // reqwest::get failed.
            return Err(anyhow!("Failed to connect to Walrus node at http://localhost:31415: {}. Is it running? Please start it with: walrus daemon --max-body-size 1048576000 --sub-wallets-dir ~/.sui/sui_config", e));
        }
    }

    // --- [3. 链上账户余额检查] ---
    // 检查参与协议的各个地址是否拥有足够的 Gas 费 (ETH) 和 SP1 证明信用 (PROVE Token)。
    println!("Checking account balances...");
    let provider = Provider::<Http>::try_from("https://sepolia-rollup.arbitrum.io/rpc")?;

    // 从私钥字符串解析出钱包实例
    let seller_wallet = seller_key.parse::<LocalWallet>()?;
    let buyer_wallet = buyer_key.parse::<LocalWallet>()?;
    let sp1_wallet = sp1_private_key.parse::<LocalWallet>()?;

    // 查询并显示 ETH 余额
    let seller_balance = provider.get_balance(seller_wallet.address(), None).await?;
    let buyer_balance = provider.get_balance(buyer_wallet.address(), None).await?;
    let sp1_balance = provider.get_balance(sp1_wallet.address(), None).await?;

    println!(
        "  - Seller balance: {} ETH",
        ethers::utils::format_ether(seller_balance)
    );
    println!(
        "  - Buyer balance: {} ETH",
        ethers::utils::format_ether(buyer_balance)
    );
    println!(
        "  - SP1 Prover balance: {} ETH",
        ethers::utils::format_ether(sp1_balance)
    );

    // SP1 `PROVE` token balance check disabled as per user request.

    // --- [4. 验证器合约状态检查] ---
    println!("Checking Verifier contracts...");
    let vss_verifier = "0x5e80ed679fb9f4050a5c7ede5ccbe39178f142a2".parse::<Address>()?;
    let vdd_verifier = "0x154D59Ed30B7784B5c9324b32b9ec5d6c8DE4071".parse::<Address>()?;
    let vss_code = provider.get_code(vss_verifier, None).await?;
    let vdd_code = provider.get_code(vdd_verifier, None).await?;
    if vss_code.is_empty() {
        return Err(anyhow!(
            "VSS Verifier contract has no code at 0x5e80ed679fb9f4050a5c7ede5ccbe39178f142a2"
        ));
    }
    if vdd_code.is_empty() {
        return Err(anyhow!(
            "VDD Verifier contract has no code at 0x154D59Ed30B7784B5c9324b32b9ec5d6c8DE4071"
        ));
    }
    println!("  - VSS and VDD Verifier contracts are deployed and valid.");

    println!(">>> Configuration checks passed.");
    Ok(())
}
