use anyhow::{Result, anyhow};
use ethers::prelude::*;
use std::env;

pub async fn run_config_checks() -> Result<()> {
    println!(">>> Running Configuration Checks...");

    // 1. Check Environment Variables
    println!("Checking environment variables...");
    let seller_key = env::var("SELLER_KEY").map_err(|_| anyhow!("SELLER_KEY not set"))?;
    let buyer_key = env::var("BUYER_KEY").map_err(|_| anyhow!("BUYER_KEY not set"))?;
    let sp1_private_key = env::var("SP1_PRIVATE_KEY").map_err(|_| anyhow!("SP1_PRIVATE_KEY not set"))?;
    println!("  - Environment variables are set.");

    // 2. Check Walrus Node Status
    println!("Checking Walrus node status...");
    match reqwest::get("http://localhost:31415").await {
        Ok(res) if res.status().is_success() => {
            println!("  - Walrus node is running.");
        }
        _ => {
            return Err(anyhow!("Walrus node is not running. Please start it with: walrus daemon --max-body-size 1048576000 --sub-wallets-dir ~/.sui/sui_config"));
        }
    }

    // 3. Check Account Balances
    println!("Checking account balances...");
    let provider = Provider::<Http>::try_from("https://sepolia-rollup.arbitrum.io/rpc")?;
    
    let seller_wallet = seller_key.parse::<LocalWallet>()?;
    let buyer_wallet = buyer_key.parse::<LocalWallet>()?;
    let sp1_wallet = sp1_private_key.parse::<LocalWallet>()?;

    let seller_balance = provider.get_balance(seller_wallet.address(), None).await?;
    let buyer_balance = provider.get_balance(buyer_wallet.address(), None).await?;
    let sp1_balance = provider.get_balance(sp1_wallet.address(), None).await?;

    println!("  - Seller balance: {} ETH", ethers::utils::format_ether(seller_balance));
    println!("  - Buyer balance: {} ETH", ethers::utils::format_ether(buyer_balance));
    println!("  - SP1 Prover balance: {} ETH", ethers::utils::format_ether(sp1_balance));

    // Check PROVE token balance
    let prove_token_address = "0x6bef15d938d4e72056ac92ea4bdd0d76b1c4ad29".parse::<Address>()?;
    let prove_token_contract = Ierc20::new(prove_token_address, std::sync::Arc::new(provider));
    let sp1_prove_balance = prove_token_contract.balance_of(sp1_wallet.address()).call().await?;
    println!("  - SP1 Prover PROVE token balance: {}", ethers::utils::format_units(sp1_prove_balance, 18)?);


    println!(">>> Configuration checks passed.");
    Ok(())
}

abigen!(
    Ierc20,
    r#"[
        function balanceOf(address account) external view returns (uint256)
    ]"#,
);
