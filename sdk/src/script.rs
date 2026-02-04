use anyhow::Result;
use ethers::prelude::*;
use std::sync::Arc;
use std::fs;
use std::env;
use dotenv::dotenv;

// 物理保留：存储 Trait 与配置
use storage::{WalrusClient, BlobStatus, StorageNetwork, WalrusConfig};

// --- [卖家核心配置 - 物理唯一源头] ---
const SELLER_VSS_SECRET: [u8; 32] = [
    0x6d, 0x61, 0x65, 0x6e, 0x61, 0x64, 0x5f, 0x74, 
    0x65, 0x73, 0x74, 0x5f, 0x73, 0x65, 0x63, 0x72, 
    0x65, 0x74, 0x5f, 0x6b, 0x65, 0x79, 0x5f, 0x30, 
    0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x31
];

fn derive_vss_pubkey(secret: &[u8; 32]) -> Vec<u8> {
    let hash = blake3::hash(secret);
    hash.as_bytes().to_vec() 
}

// --- [合约环境] ---
const HUB_ADDRESS: &str = "0x2F0E2DeA5385e8Ea5234ea5c1f46A255fC330b5F";
const RPC_URL: &str = "https://sepolia-rollup.arbitrum.io/rpc";
const ARBITRUM_SEPOLIA_CHAIN_ID: u64 = 421614;

// 锁定基准 ABI
abigen!(
    ExchangeHubContract,
    r#"[
        {
            "inputs": [
                {
                    "components": [{"internalType": "bytes", "name": "data", "type": "bytes"}],
                    "internalType": "struct Types.Pubkey",
                    "name": "ownerPubKey",
                    "type": "tuple"
                }
            ],
            "name": "createExchangeChannel",
            "outputs": [{"internalType": "address", "name": "", "type": "address"}],
            "stateMutability": "nonpayable",
            "type": "function"
        },
        {
            "anonymous": false,
            "inputs": [
                {"indexed": true, "internalType": "address", "name": "owner", "type": "address"},
                {"indexed": true, "internalType": "address", "name": "channel", "type": "address"}
            ],
            "name": "ExchangeChannelCreated",
            "type": "event"
        }
    ]"#;
    ExchangeChannelContract,
    r#"[
        {"inputs":[],"name":"nonce","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},
        {"inputs":[{"components":[{"internalType":"bytes","name":"data","type":"bytes"},{"internalType":"bytes32","name":"data_id","type":"bytes32"}],"internalType":"struct Types.DataCommitment","name":"commitment","type":"tuple"},{"internalType":"uint256","name":"price","type":"uint256"},{"internalType":"string","name":"info","type":"string"}],"name":"listFile","outputs":[],"stateMutability":"nonpayable","type":"function"}
    ]"#
);

pub struct TdpMetadata {
    pub sale_id: String,
    pub blob_id: String,
    pub end_epoch: u64,
    pub nonce: u64,
    pub channel_address: String,
}

// --- [阶段 1] 卖家：注册身份与挂牌 ---
pub async fn stage_1_seller_list(
    walrus: WalrusClient,
    eth_signer: Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
) -> Result<TdpMetadata> {
    println!("\n>>> [STAGE 1] STARTING SELLER REGISTRATION FLOW");

    let seller_pubkey = derive_vss_pubkey(&SELLER_VSS_SECRET);
    println!(">>> [DEBUG] Crypto: Derived Public Key: 0x{}", hex::encode(&seller_pubkey));

    // 1. Walrus 物理上传输出 (基准保留)
    let asset_path = "Mo.mp4";
    let file_data = fs::read(asset_path).map_err(|_| anyhow::anyhow!("File Mo.mp4 not found"))?;
    println!(">>> [DEBUG] Walrus: Uploading 18MB payload...");
    let blob_id = walrus.upload_blob(file_data.into(), Some("4")).await?;
    println!(">>> [DEBUG] Walrus: BlobID generated -> {}", blob_id.0);

    let status = walrus.get_status(&blob_id).await?;
    let end_epoch = match status {
        BlobStatus::Info { end_epoch, .. } => {
            println!(">>> [DEBUG] Walrus: EndEpoch confirmed at {}", end_epoch);
            end_epoch
        },
        _ => return Err(anyhow::anyhow!("Blob not confirmed yet.")),
    };

    // 2. 核心逻辑：探测或创建 ExchangeChannel
    println!(">>> [DEBUG] Step 3: Resolving ExchangeChannel...");
    
    let mut channel_address: Address = Address::zero();

    // 尝试从环境变量读取
    if let Ok(addr_str) = env::var("EXCHANGE_CHANNEL_ADDRESS") {
        if !addr_str.is_empty() {
            channel_address = addr_str.parse().map_err(|_| anyhow::anyhow!("Invalid address in .env"))?;
            println!(">>> [DEBUG] Found existing Channel in .env: {:?}", channel_address);
        }
    }

    if channel_address.is_zero() {
        println!(">>> [DEBUG] No existing Channel found. Creating new one via Hub...");
        let hub_addr: Address = HUB_ADDRESS.parse()?;
        let hub = ExchangeHubContract::new(hub_addr, eth_signer.clone());

        // 基准熔断检查 (PRE-FLIGHT)
        let opk = Pubkey { data: seller_pubkey.into() };
        let call = hub.create_exchange_channel(opk);
        let calldata_hex = hex::encode(&call.tx.data().expect("Failed to get calldata").0);
        
        println!(">>> [PRE-FLIGHT] Full Calldata: 0x{}", calldata_hex);
        if !calldata_hex.starts_with("34cdaf40") {
            panic!("FATAL: Selector mismatch! Expected 0x34cdaf40, got 0x{}. Gas protected.", &calldata_hex[..8]);
        }

        let tx_receipt = call.gas(1500000).send().await?.await?
            .ok_or_else(|| anyhow::anyhow!("Hub transaction failed"))?;

        for log in &tx_receipt.logs {
            if let Ok(event) = hub.decode_event::<ExchangeChannelCreatedFilter>("ExchangeChannelCreated", log.topics.clone(), log.data.clone()) {
                channel_address = event.channel;
                break;
            }
        }
        
        if channel_address.is_zero() { return Err(anyhow::anyhow!("Failed to parse new Channel address")); }
        println!(">>> [DEBUG] NEW Channel deployed at: {:?}", channel_address);
        println!(">>> [ACTION] Please manually update EXCHANGE_CHANNEL_ADDRESS={:?} in your .env", channel_address);
    }

    // 3. 计算 SaleID (基准逻辑)
    println!(">>> [DEBUG] Step 4: Syncing state for SaleID...");
    let channel = ExchangeChannelContract::new(channel_address, eth_signer.clone());
    let current_nonce = channel.nonce().call().await?;
    let chain_id = eth_signer.get_chainid().await?;

    let mut packed = Vec::new();
    packed.extend_from_slice(channel_address.as_bytes()); 
    let mut cb = [0u8; 32];
    U256::from(chain_id.as_u64()).to_big_endian(&mut cb);
    packed.extend_from_slice(&cb); 
    let mut nb = [0u8; 32];
    current_nonce.to_big_endian(&mut nb);
    packed.extend_from_slice(&nb); 

    let sale_id_hex = format!("0x{}", hex::encode(ethers::utils::keccak256(packed)));
    println!(">>> [DEBUG] Computed SaleID: {}", sale_id_hex);

    Ok(TdpMetadata {
        sale_id: sale_id_hex,
        blob_id: blob_id.0,
        end_epoch,
        nonce: current_nonce.as_u64(),
        channel_address: format!("{:?}", channel_address),
    })
}

// --- [阶段 2 & 3 物理基准保留] ---
pub async fn stage_2_buyer_buy(_signer: Arc<SignerMiddleware<Provider<Http>, LocalWallet>>, meta: &TdpMetadata) -> Result<()> {
    println!("\n>>> [STAGE 2] BUYER PURCHASE LOGIC");
    println!(">>> [DEBUG] Working with SaleID: {}", meta.sale_id);
    Ok(())
}

pub async fn stage_3_seller_finalize(_signer: Arc<SignerMiddleware<Provider<Http>, LocalWallet>>, meta: &TdpMetadata) -> Result<()> {
    println!("\n>>> [STAGE 3] SELLER FINALIZE LOGIC");
    println!(">>> [DEBUG] Finalizing for Channel: {}", meta.channel_address);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_main() -> Result<()> {
        dotenv().ok();
        let private_key = env::var("ETH_PRIVATE_KEY").expect("Need PK");
        let provider = Provider::<Http>::try_from(RPC_URL)?;
        let wallet = private_key.parse::<LocalWallet>()?.with_chain_id(ARBITRUM_SEPOLIA_CHAIN_ID);
        let signer = Arc::new(SignerMiddleware::new(provider, wallet));

        let walrus_client = WalrusClient::new(WalrusConfig {
            publisher_url: "http://127.0.0.1:31415".into(),
            aggregator_url: "http://127.0.0.1:31415".into(),
            blockberry_base: "https://api.blockberry.one/walrus-mainnet".into(),
            api_key: "eNx0cS4PemfQtVaArXbRbHcyJTnP0l".into(),
            send_object_to: None,
        });

        let meta = stage_1_seller_list(walrus_client, signer.clone()).await?;
        stage_2_buyer_buy(signer.clone(), &meta).await?;
        stage_3_seller_finalize(signer.clone(), &meta).await?;

        println!("\n[SUCCESS] TDP Sequence Complete.");
        Ok(())
    }
}