use anyhow::{Result, anyhow};
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
    let sp1_private_key = env::var("SP1_PRIVATE_KEY").map_err(|_| anyhow!("SP1_PRIVATE_KEY not set"))?;
    println!("  - Environment variables (SELLER_KEY, BUYER_KEY, SP1_PRIVATE_KEY) are set.");

    // --- [2. 本地 Walrus 节点状态检查] ---
    // 通过一个简单的 HTTP GET 请求来探测本地 Walrus 节点的健康状况。
    // Walrus 节点是本系统中的去中心化存储层，负责托管加密后的数据。
    println!("Checking Walrus node status...");
    match reqwest::get("http://localhost:31415").await {
        Ok(res) if res.status().is_success() => {
            println!("  - Walrus node is running.");
        }
        _ => {
            // 如果节点未运行，提供清晰的启动指令。
            return Err(anyhow!("Walrus node is not running. Please start it with: walrus daemon --max-body-size 1048576000 --sub-wallets-dir ~/.sui/sui_config"));
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

    println!("  - Seller balance: {} ETH", ethers::utils::format_ether(seller_balance));
    println!("  - Buyer balance: {} ETH", ethers::utils::format_ether(buyer_balance));
    println!("  - SP1 Prover balance: {} ETH", ethers::utils::format_ether(sp1_balance));

    // 查询 SP1 `PROVE` 代币余额
    // PROVE 代币是支付 SP1 证明网络费用的凭证。余额不足会导致证明提交失败。
    // 用户需要通过 Etherscan 上的 `permitAndDeposit` 交易进行充值。
    let prove_token_address = "0x6bef15d938d4e72056ac92ea4bdd0d76b1c4ad29".parse::<Address>()?;
    let prove_token_contract = Ierc20::new(prove_token_address, std::sync::Arc::new(provider));
    let sp1_prove_balance = prove_token_contract.balance_of(sp1_wallet.address()).call().await?;
    println!("  - SP1 Prover PROVE token balance: {}", ethers::utils::format_units(sp1_prove_balance, 18)?);
    if sp1_prove_balance < U256::from(10u128.pow(18)) { // 假设至少需要 10 PROVE
        println!("  - WARNING: SP1 PROVE token balance is low. Please top up at https://etherscan.io/tx/0x506d744f771e5253556ae9154ecaa26d081a7e66da380933541a88d90c0202a1");
    }

    println!(">>> Configuration checks passed.");
    Ok(())
}

// 定义 ERC20 `balanceOf` 函数的最小 ABI 接口，以便进行链上查询。
abigen!(
    Ierc20,
    r#"[
        function balanceOf(address account) external view returns (uint256)
    ]"#,
);
