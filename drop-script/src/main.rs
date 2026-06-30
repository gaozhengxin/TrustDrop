use anyhow::{anyhow, ensure, Result};
use ethers::abi::{encode, Token};
use ethers::prelude::*;
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::SecretKey;
use std::{
    env, fs,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::io::AsyncReadExt;

// 物理导入 Trait：启用 WalrusClient 异步流读取
use storage::{BlobId, StorageNetwork, WalrusClient, WalrusConfig};

use dotenv::dotenv;
use drop_lib::ecies;
use drop_lib::kdf::key_derive;

mod config_check;

// 重构后的内部模块引用
use drop_sdk::chacha8::{chacha8_decrypt, chacha8_encrypt};
// use drop_sdk::proof::{run_vdd_proof};
use drop_lib::rslh_ve::{
    create_honest_proof, derive_rslh_nonce, DEFAULT_SAMPLE_COUNT, SYMBOL_SIZE,
};
use drop_sdk::walrus::{compute_rs_id, upload_data_idempotent};
use sha2::{Digest, Sha256};
use sp1_sdk::{
    network::NetworkMode, Elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey,
    SP1Stdin,
};

// ABI 引用
use drop_sdk::abi::exchange_channel_contract as channel_abi;
use drop_sdk::abi::exchange_hub_contract as hub_abi;

abigen!(
    VSSVerifierContract,
    r#"[
    function verifyVSS(bytes calldata proof, bytes calldata publicValues, bytes32 bindingHash) external returns (bool)
]"#
);

abigen!(
    VDDVerifierContract,
    r#"[
    function verifyVDD(bytes calldata proof, bytes calldata publicValues, bytes32 bindingHash) external returns (bool)
]"#
);

use drop_sdk::abi::{DataKeySharedFilter, ExchangeChannelCreatedFilter};

const VSS_ELF: Elf = Elf::Static(include_bytes!(
    "../../guest/vss/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/vss-program"
));
const VDD_ELF: Elf = Elf::Static(include_bytes!("../../guest/vdd/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/program-vdd-walrus-rslhve"));

// --- [物理常量定义] ---
pub const INPUT_ASSET_NAME: &str =
    "KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile.mp4";
pub const RECOVERED_ASSET_NAME: &str =
    "KSC-19690716-MH-NAS01-0001-Apollo_11_Historical_Footage_and_Broll-DVC_1560~mobile-recovered.mp4";

pub const ARBITRUM_SEPOLIA_RPC: &str = "https://sepolia-rollup.arbitrum.io/rpc";
pub const WALRUS_LOCAL_ENDPOINT: &str = "http://localhost:31415";

pub const HUB_ADDRESS: &str = "0x2e506eF3F3cE222F276ddA64Df239CEF92683a78";
pub const ARBITRUM_SEPOLIA_CHAIN_ID: u64 = 421614;
pub const LIVING_WINDOW_SECS: u64 = 7 * 24 * 3600;
pub const ORACLE_TIMEOUT_SECS: u64 = 30 * 60;

pub const VSS_VERIFIER_ADDRESS: &str = "0x5e80ed679fb9f4050a5c7ede5ccbe39178f142a2";
pub const VDD_VERIFIER_ADDRESS: &str = "0x154D59Ed30B7784B5c9324b32b9ec5d6c8DE4071";

fn env_or_default(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

pub fn configured_input_asset_name() -> String {
    env_or_default("DROP_SCRIPT_INPUT_ASSET", INPUT_ASSET_NAME)
}

pub fn configured_rpc_url() -> String {
    env_or_default("ARBITRUM_SEPOLIA_RPC", ARBITRUM_SEPOLIA_RPC)
}

pub fn configured_walrus_endpoint() -> String {
    env_or_default("WALRUS_LOCAL_ENDPOINT", WALRUS_LOCAL_ENDPOINT)
}

pub fn configured_hub_address() -> Result<Address> {
    env_or_default("HUB_ADDRESS", HUB_ADDRESS)
        .parse::<Address>()
        .map_err(|_| anyhow!("Invalid HUB_ADDRESS"))
}

pub fn configured_vss_verifier_address() -> Result<Address> {
    env_or_default("VSS_VERIFIER_ADDRESS", VSS_VERIFIER_ADDRESS)
        .parse::<Address>()
        .map_err(|_| anyhow!("Invalid VSS_VERIFIER_ADDRESS"))
}

pub fn configured_vdd_verifier_address() -> Result<Address> {
    env_or_default("VDD_VERIFIER_ADDRESS", VDD_VERIFIER_ADDRESS)
        .parse::<Address>()
        .map_err(|_| anyhow!("Invalid VDD_VERIFIER_ADDRESS"))
}

fn configured_oracle_mode() -> String {
    env::var("ORACLE_MODE").unwrap_or_else(|_| "external".to_string())
}

fn configured_oracle_worker_url() -> Result<String> {
    env::var("ORACLE_WORKER_URL").map_err(|_| {
        anyhow!("ORACLE_WORKER_URL is required when ORACLE_MODE=centralized")
    })
}

fn configured_oracle_worker_token() -> Result<String> {
    env::var("ORACLE_WORKER_TOKEN").map_err(|_| {
        anyhow!("ORACLE_WORKER_TOKEN is required when ORACLE_MODE=centralized")
    })
}

fn configured_oracle_worker_status_url(worker_url: &str) -> String {
    env::var("ORACLE_WORKER_STATUS_URL")
        .unwrap_or_else(|_| format!("{}/status", worker_url.trim_end_matches('/')))
}

#[derive(Debug, Clone)]
pub struct ListingState {
    pub unique_sale_id: [u8; 32],
    pub onchain_data_version: [u8; 32],
    pub walrus_blob_id: String,
    pub channel_address: Address,
    pub original_asset_id: [u8; 32],
    pub encrypted_blob_id: [u8; 32],
    pub original_len: usize,
}

#[derive(Debug, Clone)]
pub struct PurchaseState {
    pub secret_sharing_key: [u8; 32],
    pub transaction_hash: H256,
    pub ephemeral_pubkey: Vec<u8>,
}

fn seller_public_key_bytes(sk_bytes: &[u8; 32]) -> Result<Vec<u8>> {
    let sk = SecretKey::from_slice(sk_bytes)
        .map_err(|e| anyhow!("Invalid seller VSS secret key: {}", e))?;
    Ok(sk.public_key().to_encoded_point(true).as_bytes().to_vec())
}

fn data_key_commitment(asset_key: &[u8; 32]) -> [u8; 32] {
    *blake3::hash(asset_key).as_bytes()
}

fn compute_vss_binding_hash(
    data_key_commitment: [u8; 32],
    vss_key_commitment: [u8; 32],
    encrypted_data_key: [u8; 32],
) -> [u8; 32] {
    ethers::utils::keccak256(encode(&[
        Token::FixedBytes(data_key_commitment.to_vec()),
        Token::Array(vec![Token::FixedBytes(vss_key_commitment.to_vec())]),
        Token::Array(vec![Token::FixedBytes(encrypted_data_key.to_vec())]),
    ]))
}

fn compute_vdd_binding_hash(
    c_origin: &[u8],
    data_key_commitment: [u8; 32],
    c_cipher: &[u8],
) -> [u8; 32] {
    ethers::utils::keccak256(encode(&[
        Token::Bytes(c_origin.to_vec()),
        Token::FixedBytes(data_key_commitment.to_vec()),
        Token::Bytes(c_cipher.to_vec()),
    ]))
}

fn validate_vdd_public_values(
    public_values: &Bytes,
    c_origin: [u8; 32],
    c_key: [u8; 32],
    c_cipher: [u8; 32],
) -> Result<()> {
    let bytes = public_values.as_ref();
    ensure!(
        bytes.len() == 96,
        "VDD public values length mismatch: {}",
        bytes.len()
    );
    ensure!(
        &bytes[0..32] == c_origin.as_slice(),
        "VDD public c_origin mismatch"
    );
    ensure!(
        &bytes[32..64] == c_key.as_slice(),
        "VDD public c_key mismatch"
    );
    ensure!(
        &bytes[64..96] == c_cipher.as_slice(),
        "VDD public c_cipher mismatch"
    );
    Ok(())
}

fn validate_vss_public_values(
    public_values: &Bytes,
    expected_cipher_block: [u8; 32],
    expected_key_commitment: [u8; 32],
    expected_nonce: [u8; 12],
) -> Result<()> {
    let decoded = drop_lib::common::decode_public_outputs_with_cipher(public_values.as_ref())
        .map_err(|e| anyhow!("Failed to decode VSS public values: {}", e))?;
    ensure!(
        decoded.length == 1,
        "VSS public length mismatch: {}",
        decoded.length
    );
    ensure!(
        decoded.cipher_block.first() == Some(&expected_cipher_block),
        "VSS public encrypted data key mismatch"
    );
    ensure!(
        decoded.h_k_commitment.first() == Some(&expected_key_commitment),
        "VSS public key commitment mismatch"
    );
    ensure!(
        decoded.nonce.first() == Some(&expected_nonce),
        "VSS public nonce mismatch"
    );
    Ok(())
}

// ==========================================================
// --- [第一部分：原子工具函数] ---
// ==========================================================

/// # [TOOL] 计算唯一销售 ID (Sale ID)
///
/// ## 作用
/// 根据通道地址、链 ID 和当前 nonce 生成一个全局唯一的销售标识符。
/// 这个 ID 用于在 `purchase` 和 `fulfill` 阶段精确指向一个特定的销售事件，防止重放攻击或混淆不同的交易。
///
/// ## 输入
/// - `channel_address`: 销售通道的合约地址
/// - `chain_id`: 当前区块链的 ID
/// - `nonce`: 通道合约中记录的当前 nonce，每次销售递增
///
/// ## 输出
/// - `[u8; 32]`: Keccak256 哈希结果，作为唯一 ID
pub fn compute_sale_id(channel_address: Address, chain_id: u64, nonce: U256) -> [u8; 32] {
    let mut packed_bytes = Vec::new();
    packed_bytes.extend_from_slice(channel_address.as_bytes());

    // Correctly encode chain_id as uint256 (32 bytes)
    let mut chain_id_bytes = [0u8; 32];
    U256::from(chain_id).to_big_endian(&mut chain_id_bytes);
    packed_bytes.extend_from_slice(&chain_id_bytes);

    let mut nonce_bytes = [0u8; 32];
    nonce.to_big_endian(&mut nonce_bytes);
    packed_bytes.extend_from_slice(&nonce_bytes);
    ethers::utils::keccak256(packed_bytes).into()
}

/// # [TOOL] 获取或创建交易通道
///
/// ## 角色
/// 卖家
///
/// ## 作用
/// 调用 Hub 合约的 `createExchangeChannel` 方法来创建一个新的个人交易通道。
/// 如果已经存在，理论上可以复用，但这里为了演示总是创建一个新的。
///
/// ## 输入
/// - `signer`: 卖家的钱包签名器
///
/// ## 输出
/// - `Address`: 新创建的通道合约地址
pub async fn get_or_create_channel(
    signer: Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
    seller_vss_pubkey: Vec<u8>,
) -> Result<Address> {
    let hub_addr = configured_hub_address()?;
    let hub_contract = hub_abi::ExchangeHubContract::new(hub_addr, signer.clone());
    let initial_vss_pubkey = hub_abi::Pubkey {
        data: seller_vss_pubkey.into(),
    };

    let receipt = hub_contract
        .create_exchange_channel(initial_vss_pubkey)
        .send()
        .await?
        .await?
        .ok_or(anyhow!("Hub fail"))?;
    println!(
        ">>> createExchangeChannel txHash: {:#x}",
        receipt.transaction_hash
    );

    for log in receipt.logs.iter().filter(|log| log.address == hub_addr) {
        let Ok(ev) = hub_contract.decode_event::<ExchangeChannelCreatedFilter>(
            "ExchangeChannelCreated",
            log.topics.clone(),
            log.data.clone(),
        ) else {
            continue;
        };

        if ev.owner != signer.address() {
            continue;
        }

        ensure!(
            hub_contract
                .is_registered_channel(ev.channel)
                .call()
                .await?,
            "Created channel is not registered in hub: {:?}",
            ev.channel
        );
        return Ok(ev.channel);
    }

    Err(anyhow!(
        "No ExchangeChannelCreated event found for owner {:?}",
        signer.address()
    ))
}

/// # [TOOL] 从购买事件中解析信息
///
/// ## 角色
/// 卖家或任何需要验证购买信息的人
///
/// ## 作用
/// 根据 `purchase` 交易的哈希，从链上日志中解析出买家地址和具体的交易信息 (`ExchangeInfo`)。
/// 这是卖家履行订单（`fulfill`）前获取关键信息（如数据承诺、VSS 密钥承诺等）的必要步骤。
///
/// ## 输入
/// - `client`: Ethers Provider 实例
/// - `transaction_hash`: `purchase` 交易的哈希
/// - `channel_address`: 发生购买的通道地址
///
/// ## 输出
/// - `(Address, channel_abi::ExchangeInfo)`: 买家地址和该次购买的详细信息
pub async fn get_purchase_info_from_event(
    client: &Provider<Http>,
    transaction_hash: H256,
    channel_address: Address,
    sale_id: [u8; 32],
) -> Result<(Address, channel_abi::ExchangeInfo)> {
    let hub_addr = configured_hub_address()?;
    let receipt = client
        .get_transaction_receipt(transaction_hash)
        .await?
        .ok_or(anyhow!("TX missing"))?;

    let hub_inst = hub_abi::ExchangeHubContract::new(hub_addr, Arc::new(client.clone()));

    let mut purchase_ev = None;
    for log in receipt.logs.iter().filter(|log| log.address == hub_addr) {
        let Ok(ev) = hub_inst.decode_event::<hub_abi::PurchaseEventFilter>(
            "PurchaseEvent",
            log.topics.clone(),
            log.data.clone(),
        ) else {
            continue;
        };

        if ev.channel != channel_address {
            continue;
        }
        if ev.sale_id != sale_id {
            continue;
        }
        if ev.exchange_info.sale_digest != sale_id {
            return Err(anyhow!(
                "PurchaseEvent sale id mismatch: event saleId=0x{}, exchangeInfo.saleDigest=0x{}",
                hex::encode(ev.sale_id),
                hex::encode(ev.exchange_info.sale_digest)
            ));
        }
        purchase_ev = Some(ev);
        break;
    }

    let purchase_ev = purchase_ev.ok_or_else(|| {
        anyhow!(
            "No matching PurchaseEvent found from Hub for channel {:?}, saleId 0x{}",
            channel_address,
            hex::encode(sale_id)
        )
    })?;

    let h = purchase_ev.exchange_info;
    let channel_info = channel_abi::ExchangeInfo {
        sale_digest: h.sale_digest,
        price: h.price,
        init_time: h.init_time,
        deadline: h.deadline,
        data_commitment: h.data_commitment,
        vss_key_commitment: h.vss_key_commitment,
    };

    Ok((purchase_ev.buyer, channel_info))
}

/// # [TOOL] 模拟 VSS 验证
pub async fn simulate_vss_verify(
    provider: &Provider<Http>,
    verifier_address: Address,
    vk_string: String,
    binding_hash: [u8; 32],
    public_values: Bytes,
    proof_bytes: Bytes,
) -> Result<()> {
    println!(
        ">>> [SIMULATION] Testing VSS verification on Verifier: {}",
        verifier_address
    );
    println!("  - Input VK: {}", vk_string);
    println!(
        "  - Input Public Values: 0x{}",
        hex::encode(public_values.to_vec())
    );
    println!("  - Input Proof (len): {} bytes", proof_bytes.len());
    println!("  - Input Proof: 0x{}", hex::encode(proof_bytes.to_vec()));

    let contract = VSSVerifierContract::new(verifier_address, Arc::new(provider.clone()));
    let call_builder =
        contract.verify_vss(proof_bytes.clone(), public_values.clone(), binding_hash);

    if let Some(calldata) = call_builder.calldata() {
        println!(
            ">>> [DEBUG] VSS Transaction Calldata: 0x{}",
            hex::encode(&calldata)
        );
    }

    match call_builder.call().await {
        Ok(true) => {
            println!(">>> [SIMULATION] VSS Proof verified successfully!");
            Ok(())
        }
        Ok(false) => Err(anyhow!("VSS Proof FAILED. Verifier returned false.")),
        Err(e) => {
            if let Some(revert_data) = e.as_revert() {
                let hex_revert = hex::encode(revert_data);
                println!(">>> [DEBUG] Raw Revert Data: 0x{}", hex_revert);
                let selector = if hex_revert.len() >= 8 {
                    &hex_revert[0..8]
                } else {
                    &hex_revert
                };
                let meaning = match selector {
                    "7fcdd1f4" => "InvalidPublicValues() - SP1 Verifier rejected the public values. This almost always means the VK hardcoded in your deployed verifier contract is stale and doesn't match the newly compiled guest program!",
                    "09bde339" => "InvalidProof() - The ZK proof itself failed mathematical verification.",
                    "1b50428d" => "WrongVerificationKey() - The VK does not match the one expected by the Verifier.",
                    _ => "Unknown Custom Error",
                };
                let err_msg = format!(
                    "VSS Proof FAILED. Details: Revert(0x{}) -> {}",
                    selector, meaning
                );
                println!(">>> [SIMULATION] {}", err_msg);
                Err(anyhow!(err_msg))
            } else {
                let err_msg = format!("VSS Proof FAILED. Details: {:?}", e);
                println!(">>> [SIMULATION] {}", err_msg);
                Err(anyhow!(err_msg))
            }
        }
    }
}

/// # [TOOL] 模拟 VDD 验证
pub async fn simulate_vdd_verify(
    provider: &Provider<Http>,
    verifier_address: Address,
    vk_string: String,
    binding_hash: [u8; 32],
    public_values: Bytes,
    proof_bytes: Bytes,
) -> Result<()> {
    println!(
        ">>> [SIMULATION] Testing VDD verification on Verifier: {}",
        verifier_address
    );
    println!("  - Input VK: {}", vk_string);
    println!(
        "  - Input Public Values: 0x{}",
        hex::encode(public_values.to_vec())
    );
    println!("  - Input Proof (len): {} bytes", proof_bytes.len());

    let contract = VDDVerifierContract::new(verifier_address, Arc::new(provider.clone()));
    let call_builder =
        contract.verify_vdd(proof_bytes.clone(), public_values.clone(), binding_hash);

    if let Some(calldata) = call_builder.calldata() {
        println!(
            ">>> [DEBUG] VDD Transaction Calldata: 0x{}",
            hex::encode(&calldata)
        );
    }

    match call_builder.call().await {
        Ok(true) => {
            println!(">>> [SIMULATION] VDD Proof verified successfully!");
            Ok(())
        }
        Ok(false) => Err(anyhow!("VDD Proof FAILED. Verifier returned false.")),
        Err(e) => {
            if let Some(revert_data) = e.as_revert() {
                let hex_revert = hex::encode(revert_data);
                println!(">>> [DEBUG] Raw Revert Data: 0x{}", hex_revert);
                let selector = if hex_revert.len() >= 8 {
                    &hex_revert[0..8]
                } else {
                    &hex_revert
                };
                let meaning = match selector {
                    "7fcdd1f4" => "InvalidPublicValues() - SP1 Verifier rejected the public values. Check if your deployed verifier VK is out of date!",
                    "09bde339" => "InvalidProof() - The ZK proof itself failed mathematical verification.",
                    "1b50428d" => "WrongVerificationKey() - The VK does not match the one expected by the Verifier.",
                    _ => "Unknown Custom Error",
                };
                let err_msg = format!(
                    "VDD Proof FAILED. Details: Revert(0x{}) -> {}",
                    selector, meaning
                );
                println!(">>> [SIMULATION] {}", err_msg);
                Err(anyhow!(err_msg))
            } else {
                let err_msg = format!("VDD Proof FAILED. Details: {:?}", e);
                println!(">>> [SIMULATION] {}", err_msg);
                Err(anyhow!(err_msg))
            }
        }
    }
}

// ==========================================================
// --- [第二部分：协议阶段实现 - 语义化重构] ---
// ==========================================================

/// # [STAGE 1] 卖家挂牌 (Listing)
///
/// ## 角色
/// 卖家
///
/// ## 流程
/// 1. **读取本地文件**: 从磁盘读取要出售的原始资产。
/// 2. **计算原始资产 ID**: 对原始文件内容进行哈希，生成 `original_asset_id`，作为数据的唯一标识。
/// 3. **加密资产**: 使用卖家的 `asset_encryption_key` 通过 ChaCha8 算法加密文件。
/// 4. **上传加密数据**: 将加密后的文件上传到 Walrus 去中心化存储网络，并获取其 `walrus_blob_id`。
/// 5. **创建/获取通道**: 在链上创建或获取一个个人交易通道。
/// 6. **生成唯一销售 ID**: 结合通道信息和 nonce 生成本次挂牌的 `unique_sale_id`。
/// 7. **链上挂牌**: 调用通道合约的 `listFile` 方法，将数据承诺 (`original_asset_id`)、价格等信息记录上链。
///
/// ## 输出
/// - `unique_sale_id`: 本次销售的唯一链上 ID。
/// - `onchain_data_version`: 原始资产 ID 的链上版本（哈希）。
/// - `walrus_blob_id`: 加密数据在 Walrus 上的存储 ID。
/// - `channel_addr`: 交易通道地址。
/// - `original_asset_id`: 原始数据的哈希 ID。
/// - `encrypted_blob_id`: 加密数据的哈希 ID。
pub async fn stage_1_listing(walrus: &WalrusClient, ctx: &SellerContext) -> Result<ListingState> {
    println!(">>> [STAGE 1] LISTING...");
    let mut file_payload = fs::read(configured_input_asset_name())?;
    let original_len = file_payload.len();
    let padded_len = (original_len + SYMBOL_SIZE - 1) / SYMBOL_SIZE * SYMBOL_SIZE;
    file_payload.resize(padded_len, 0);

    let original_asset_id = compute_rs_id(&file_payload)?;

    let asset_nonce = derive_rslh_nonce(&ctx.asset_encryption_key, b"maenad_v1");
    let encrypted_asset_data =
        chacha8_encrypt(&file_payload, &ctx.asset_encryption_key, &asset_nonce, 0)?;
    let encrypted_blob_id = compute_rs_id(&encrypted_asset_data)?;
    println!("  - original_asset_id: 0x{}", hex::encode(original_asset_id));
    println!("  - encrypted_blob_id: 0x{}", hex::encode(encrypted_blob_id));
    println!("  - original_len: {}", original_len);
    println!("  - padded_len: {}", padded_len);

    let walrus_blob_id = upload_data_idempotent(walrus, encrypted_asset_data).await?;
    println!("  - walrus_blob_id: {}", walrus_blob_id);

    let channel_addr = get_or_create_channel(
        ctx.signer.clone(),
        seller_public_key_bytes(&ctx.owner_sk_bytes)?,
    )
    .await?;
    println!("  - channel_address: {:?}", channel_addr);
    let channel_contract =
        channel_abi::ExchangeChannelContract::new(channel_addr, ctx.signer.clone());

    let sale_nonce = channel_contract.nonce().call().await?;
    let unique_sale_id = compute_sale_id(channel_addr, ARBITRUM_SEPOLIA_CHAIN_ID, sale_nonce);
    let onchain_data_version = ethers::utils::keccak256(original_asset_id);

    let arg_commit = channel_abi::DataCommitment {
        data: original_asset_id.to_vec().into(),
    };
    let arg_price = 10u128.pow(16).into(); // 0.01 ETH
    let arg_meta = "TrustDrop Asset v1".to_string();

    let receipt = channel_contract
        .list_file(arg_commit, arg_price, arg_meta)
        .send()
        .await?
        .await?
        .ok_or(anyhow!("listFile receipt missing"))?;
    println!("  - list_file_tx: {:#x}", receipt.transaction_hash);
    println!("  - sale_id: 0x{}", hex::encode(unique_sale_id));
    println!(
        "  - data_version: 0x{}",
        hex::encode(onchain_data_version)
    );
    Ok(ListingState {
        unique_sale_id,
        onchain_data_version: onchain_data_version.into(),
        walrus_blob_id,
        channel_address: channel_addr,
        original_asset_id,
        encrypted_blob_id,
        original_len,
    })
}

/// # [STAGE 1.5] 卖家提交数据密钥承诺
///
/// ## 角色
/// 卖家
///
/// ## 作用
/// 在 `fulfill` 之前，卖家必须调用 `submitDataKeyCommitment` 方法，将用于加密资产的 `asset_encryption_key` 的哈希承诺提交上链。
/// 这个承诺 (`dataKeyCommitment`) 后续会被 VSS 和 VDD 证明用来生成绑定哈希，确保证明与特定的数据密钥相关联。
///
/// ## 输入
/// - `ctx`: 卖家的上下文，包含钱包签名器和密钥
/// - `channel_address`: 交易通道地址
async fn stage_1_5_submit_key_commitment(
    ctx: &SellerContext,
    channel_address: Address,
) -> Result<()> {
    println!(">>> [STAGE 1.5] SUBMITTING DATA KEY COMMITMENT...");
    let channel_contract =
        channel_abi::ExchangeChannelContract::new(channel_address, ctx.signer.clone());

    let data_key_commitment = data_key_commitment(&ctx.asset_encryption_key);

    let receipt = channel_contract
        .submit_data_key_commitment(data_key_commitment.into())
        .send()
        .await?
        .await?
        .ok_or(anyhow!("submitDataKeyCommitment receipt missing"))?;

    println!(
        ">>> Data key commitment submitted: {:#x}",
        receipt.transaction_hash
    );
    Ok(())
}

/// # [STAGE 1.6] 卖家提前提交 VDD 证明
///
/// VDD 只依赖当前资产、加密资产、data key commitment 和 cCipher，不依赖具体买家。
/// 因此它应当在 publish/listing 后提前完成，fulfill 阶段只处理 buyer-bound VSS。
async fn stage_1_6_submit_vdd_proof(
    walrus_client: &WalrusClient,
    listing: &ListingState,
    ctx: &SellerContext,
) -> Result<H256> {
    println!(">>> [STAGE 1.6] SUBMITTING VDD PROOF...");
    let channel_contract =
        channel_abi::ExchangeChannelContract::new(listing.channel_address, ctx.signer.clone());
    let c_cipher = listing.encrypted_blob_id.to_vec();
    if channel_contract
        .vdd_verified(c_cipher.clone().into())
        .call()
        .await?
    {
        println!(">>> VDD proof already verified for current cCipher.");
        return Ok(H256::zero());
    }

    let data_key_commit = data_key_commitment(&ctx.asset_encryption_key);
    let vdd_binding_hash =
        compute_vdd_binding_hash(&listing.original_asset_id, data_key_commit, &c_cipher);
    let vdd_verifier_addr = configured_vdd_verifier_address()?;
    let (proof, public_values, vk) = generate_vdd_proof(
        walrus_client,
        &listing.walrus_blob_id,
        ctx,
        listing.original_asset_id,
        listing.encrypted_blob_id,
    )
    .await?;

    validate_vdd_public_values(
        &public_values,
        listing.original_asset_id,
        data_key_commit,
        listing.encrypted_blob_id,
    )?;
    simulate_vdd_verify(
        ctx.signer.provider(),
        vdd_verifier_addr,
        vk,
        vdd_binding_hash,
        public_values.clone(),
        proof.clone(),
    )
    .await?;

    let receipt = channel_contract
        .submit_vdd_proof(
            proof,
            public_values,
            listing.original_asset_id.to_vec().into(),
            c_cipher.into(),
        )
        .send()
        .await?
        .await?
        .ok_or(anyhow!("submitVDDProof receipt missing"))?;
    println!(">>> submitVDDProof txHash: {:#x}", receipt.transaction_hash);
    Ok(receipt.transaction_hash)
}

/// # [STAGE 2] 买家购买 (Purchase)
///
/// ## 角色
/// 买家
///
/// ## 流程
/// 1. **衍生共享密钥**: 使用买家固定的密钥和 `original_asset_id` 衍生出一个用于本次交易的 `secret_sharing_key`。
/// 2. **加密共享密钥**: 使用卖家的 VSS 公钥 (此处为硬编码的模拟值) 通过 ECIES 算法加密 `secret_sharing_key`。
/// 3. **计算 VSS 承诺**: 对 `secret_sharing_key` 进行哈希，生成 `vss_key_commitment`，用于后续 `fulfill` 阶段的验证。
/// 4. **链上购买**: 调用通道合约的 `purchase` 方法，并支付指定价格的 ETH。交易中包含了加密后的共享密钥、VSS 承诺和有效期等信息。
///
/// ## 输出
/// - `secret_sharing_key`: 买家本地保存的共享密钥，用于后续解密数据密钥。
/// - `purchase_transaction_hash`: 本次购买交易的哈希。
pub async fn stage_2_purchase(
    ctx: &BuyerContext,
    unique_sale_id: [u8; 32],
    onchain_data_version: [u8; 32],
    channel_address: Address,
    original_asset_id: [u8; 32],
    seller_vss_pub: &[u8],
) -> Result<PurchaseState> {
    println!(">>> [STAGE 2] PURCHASE...");
    let secret_sharing_key = key_derive(&[0xbb; 32], &original_asset_id).map_err(|e| anyhow!(e))?;
    let (encrypted_vss_key, ephemeral_pubkey) =
        ecies::encrypt(seller_vss_pub, &secret_sharing_key)?;

    let arg_deadline = U256::from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + LIVING_WINDOW_SECS + 86400,
    );
    let arg_price = 10u128.pow(16).into();
    let arg_vss_commit: [u8; 32] = *blake3::hash(&secret_sharing_key).as_bytes();

    let channel_contract =
        channel_abi::ExchangeChannelContract::new(channel_address, ctx.signer.clone());
    let tx = channel_contract
        .purchase(
            unique_sale_id,
            onchain_data_version.into(),
            arg_price,
            arg_deadline,
            original_asset_id.to_vec().into(), // Correct dataCommitment
            arg_vss_commit.into(),
            encrypted_vss_key.into(),
        )
        .value(arg_price)
        .send()
        .await?
        .await?;
    Ok(PurchaseState {
        secret_sharing_key,
        transaction_hash: tx
            .ok_or(anyhow!("purchase receipt missing"))?
            .transaction_hash,
        ephemeral_pubkey,
    })
}

/// # [STAGE 3] 卖家履行 (Fulfill)
///
/// ## 角色
/// 卖家
///
/// ## 流程
/// 1. **获取购买信息**: 从 `purchase` 交易事件中解析出买家和交易详情。
/// 2. **解密共享密钥**: 从链上获取买家提交的加密共享密钥，并用自己的 VSS 私钥解密，得到 `secret_sharing_key`。
/// 3. **封装数据密钥**: 使用解密出的 `secret_sharing_key` 加密真正的 `asset_encryption_key`。
/// 4. **生成 VSS 证明**: 调用 SP1 ZKVM 生成 VSS (Verifiable Secret Sharing) 证明。
///    - **作用**: 证明卖家正确地使用了 `secret_sharing_key` 来加密 `asset_encryption_key`。
///    - **输入**: `secret_sharing_key`, `asset_encryption_key`。
///    - **输出**: ZK 证明和公开值。
/// 5. **生成 VDD 证明**: 调用 SP1 ZKVM 生成 VDD (Verifiable Data Decryption) 证明。
///    - **作用**: 证明加密存储在 Walrus 上的数据 (`encrypted_blob_id`) 确实是由 `original_asset_id` 通过 `asset_encryption_key` 加密得来的。
///    - **输入**: `original_asset_id`, `encrypted_blob_id`, `asset_encryption_key` 等。
///    - **输出**: ZK 证明和公开值。
/// 6. **链上履行**: 调用通道合约的 `fulfill` 方法，提交封装后的数据密钥、VSS 证明和 VDD 证明。
pub async fn stage_3_fulfill(
    _walrus_client: &WalrusClient,
    listing: &ListingState,
    purchase: &PurchaseState,
    ctx: &SellerContext,
) -> Result<H256> {
    println!(">>> [STAGE 3] FULFILL...");

    let channel_contract =
        channel_abi::ExchangeChannelContract::new(listing.channel_address, ctx.signer.clone());
    let (buyer, exchange_info) = get_purchase_info_from_event(
        ctx.signer.provider(),
        purchase.transaction_hash,
        listing.channel_address,
        listing.unique_sale_id,
    )
    .await?;

    let audience_idx = channel_contract.audience_index(buyer).call().await?;
    let (_stored_vss_commitment, encrypted_vss_key) =
        channel_contract.audience_list(audience_idx).call().await?;
    let secret_sharing_key = ecies::decrypt(
        &ctx.owner_sk_bytes,
        &encrypted_vss_key,
        &purchase.ephemeral_pubkey,
    )?;
    ensure!(
        secret_sharing_key == purchase.secret_sharing_key,
        "Decrypted VSS key does not match buyer purchase context"
    );
    let wrapped_asset_key_vec = chacha8_encrypt(
        &ctx.asset_encryption_key.to_vec(),
        &secret_sharing_key,
        &[0u8; 12],
        0,
    )?;
    let wrapped_asset_key: [u8; 32] = wrapped_asset_key_vec
        .clone()
        .try_into()
        .map_err(|_| anyhow!("wrapped asset key must be exactly 32 bytes"))?;
    let data_key_commit = data_key_commitment(&ctx.asset_encryption_key);
    let vss_key_commit = exchange_info.vss_key_commitment;
    let vss_binding_hash =
        compute_vss_binding_hash(data_key_commit, vss_key_commit, wrapped_asset_key);
    // --- 使用配置的 Verifier 地址 ---
    let vss_verifier_addr = configured_vss_verifier_address()?;
    println!("  - Using VSS Verifier at: {}", vss_verifier_addr);

    // --- 生成并独立模拟 VSS 证明 ---
    let vss_result = generate_vss_proof(secret_sharing_key, ctx.asset_encryption_key).await;
    let (v_proof, v_pv) = if let Ok((proof, pv, vk)) = vss_result {
        if let Err(e) =
            validate_vss_public_values(&pv, wrapped_asset_key, vss_key_commit, [0u8; 12])
        {
            return Err(e);
        }
        if let Err(e) = simulate_vss_verify(
            ctx.signer.provider(),
            vss_verifier_addr,
            vk,
            vss_binding_hash,
            pv.clone(),
            proof.clone(),
        )
        .await
        {
            return Err(e);
        }
        (proof, pv)
    } else if let Err(e) = vss_result {
        return Err(anyhow!("VSS proof generation failed: {:?}", e));
    } else {
        (Bytes::new(), Bytes::new())
    };

    if !channel_contract
        .vdd_verified(listing.encrypted_blob_id.to_vec().into())
        .call()
        .await?
    {
        return Err(anyhow!(
            "VDD is not verified before fulfill; run stage_1_6_submit_vdd_proof first"
        ));
    }

    // --- 构造参数并发送最终交易 ---
    println!(">>> All simulations passed. Submitting fulfill transaction...");
    let arg_vss = channel_abi::Vssargs {
        encrypted_data_key: wrapped_asset_key.into(),
        proof: v_proof,
        public_values: v_pv,
    };
    let arg_vdd = channel_abi::Vddargs {
        proof: Bytes::new(),
        public_values: Bytes::new(),
        c_cipher: listing.encrypted_blob_id.to_vec().into(),
    };
    let arg_ver = listing.onchain_data_version.into();

    println!(">>> [STAGE 3] FULFILL CONTRACT CALL ARGS:");
    println!("  - buyer: {:?}", buyer);
    println!(
        "  - exchange_info.sale_digest: 0x{}",
        hex::encode(exchange_info.sale_digest)
    );
    println!("  - arg_ver: 0x{}", hex::encode(arg_ver));
    println!(
        "  - arg_vss.encrypted_data_key: 0x{}",
        hex::encode(&arg_vss.encrypted_data_key)
    );
    println!(
        "  - arg_vss.public_values: 0x{}",
        hex::encode(arg_vss.public_values.to_vec())
    );
    println!(
        "  - arg_vdd.c_cipher: 0x{}",
        hex::encode(arg_vdd.c_cipher.to_vec())
    );
    println!(
        "  - arg_vdd.public_values: 0x{}",
        hex::encode(arg_vdd.public_values.to_vec())
    );

    let receipt = channel_contract
        .fulfill(buyer, exchange_info, arg_ver, arg_vss, arg_vdd)
        .send()
        .await?
        .await?
        .ok_or(anyhow!("fulfill receipt missing"))?;
    Ok(receipt.transaction_hash)
}

/// Trigger the centralized oracle Worker after fulfill emits OracleRequested.
///
/// This is intentionally opt-in. If ORACLE_MODE is unset, the script keeps the
/// old behavior and only waits for oracleSuccessUntil.
pub async fn trigger_centralized_oracle_worker_if_enabled(fulfill_tx_hash: H256) -> Result<()> {
    let mode = configured_oracle_mode();
    if mode != "centralized" {
        println!(
            ">>> [ORACLE] ORACLE_MODE={} ; centralized Worker trigger skipped.",
            mode
        );
        return Ok(());
    }

    let worker_url = configured_oracle_worker_url()?;
    let worker_token = configured_oracle_worker_token()?;
    let status_url = configured_oracle_worker_status_url(&worker_url);
    let client = reqwest::Client::new();

    println!(">>> [ORACLE] Checking centralized Worker status...");
    let status_response = client
        .get(&status_url)
        .bearer_auth(&worker_token)
        .send()
        .await?;
    let status_code = status_response.status();
    let status_body: serde_json::Value = status_response.json().await?;
    if !status_code.is_success() || status_body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        return Err(anyhow!(
            "centralized oracle Worker status not ready: HTTP {}, body={}",
            status_code,
            status_body
        ));
    }

    println!(">>> [ORACLE] Triggering centralized Worker report...");
    let fulfill_tx_hash_hex = format!("{:#x}", fulfill_tx_hash);
    let response = client
        .post(format!("{}/oracle/fulfill", worker_url.trim_end_matches('/')))
        .bearer_auth(&worker_token)
        .json(&serde_json::json!({
            "chainId": ARBITRUM_SEPOLIA_CHAIN_ID,
            "txHash": fulfill_tx_hash_hex,
        }))
        .send()
        .await?;
    let response_status = response.status();
    let response_body: serde_json::Value = response.json().await?;
    if !response_status.is_success()
        || response_body.get("ok").and_then(|v| v.as_bool()) != Some(true)
    {
        return Err(anyhow!(
            "centralized oracle Worker fulfill failed: HTTP {}, body={}",
            response_status,
            response_body
        ));
    }

    if response_body
        .get("alreadyFulfilled")
        .and_then(|v| v.as_bool())
        == Some(true)
    {
        println!(">>> [ORACLE] Request already fulfilled.");
        return Ok(());
    }

    let report_tx_hash = response_body
        .get("reportTxHash")
        .and_then(|v| v.as_str())
        .unwrap_or("<missing>");
    println!(">>> [ORACLE] Worker report tx: {}", report_tx_hash);
    Ok(())
}

/// # [PROOF] 生成 VDD (Verifiable Data Decryption) 证明
///
/// ## 角色
/// 卖家 (证明者)
///
/// ## 作用
/// 使用 SP1 ZKVM 生成一个 ZK 证明，公开证明一个加密数据 `C` 是由一个公开的原始数据 `O` 使用一个私密密钥 `k` 加密得到的，
/// 即 `C = Enc(O, k)`。合约在链上验证此证明，确保卖家没有欺诈 (例如，上传一个无关的加密文件)。
///
/// ## 流程
/// 1. **准备数据**: 从本地和 Walrus 网络获取原始数据和加密数据。
/// 2. **构建输入 (Stdin)**: 按照 Guest 程序 (`program-vdd-walrus-rslhve`) 的要求，将原始数据承诺、加密数据承诺、密钥承诺、采样证明等数据写入 `SP1Stdin`。
/// 3. **生成诚实性采样**: 根据链上信息生成一系列随机采样点，并使用 `create_honest_proof` 创建这些点的局部同态证明，作为 ZK 电路的输入。
/// 4. **调用 Prover**:
///    - `client.setup(VDD_ELF)`: 加载 VDD Guest 程序的 ELF 文件，准备证明环境。
///    - `client.prove(...)`: 使用证明密钥和构造好的 Stdin 执行证明过程，生成 ZK 证明。
///
/// ## 输出
/// - `(Bytes, Bytes)`: ZK 证明和公开值。
pub async fn generate_vdd_proof(
    walrus_client: &WalrusClient,
    walrus_blob_id: &str,
    ctx: &SellerContext,
    original_asset_id: [u8; 32],
    encrypted_blob_id: [u8; 32],
) -> Result<(Bytes, Bytes, String)> {
    // 1. === 准备 VDD 电路所需的全部输入 ===
    let mut origin_data = fs::read(configured_input_asset_name())?;
    let original_len = origin_data.len();
    let padded_len = (original_len + SYMBOL_SIZE - 1) / SYMBOL_SIZE * SYMBOL_SIZE;
    origin_data.resize(padded_len, 0);

    let mut reader = download_walrus_blob_with_backoff(walrus_client, walrus_blob_id).await?;
    let mut cipher_data = Vec::new();
    reader.read_to_end(&mut cipher_data).await?;

    // 【重大修复】：必须使用和 stage_1_5_submit_key_commitment 相同的哈希算法
    let c_key_bytes = data_key_commitment(&ctx.asset_encryption_key);
    let aux_data = b"maenad_v1";
    let nonce = derive_rslh_nonce(&ctx.asset_encryption_key, aux_data);

    println!(">>> [VDD PROOF] zkVM Inputs:");
    println!(
        "  - original_asset_id: 0x{}",
        hex::encode(original_asset_id)
    );
    println!(
        "  - encrypted_blob_id: 0x{}",
        hex::encode(encrypted_blob_id)
    );
    println!("  - c_key_bytes: 0x{}", hex::encode(c_key_bytes));
    println!("  - aux_data: 0x{}", hex::encode(aux_data));
    println!(
        "  - asset_encryption_key: 0x{}",
        hex::encode(ctx.asset_encryption_key)
    );

    // 2. === 构建 SP1 Stdin ===
    let mut stdin = SP1Stdin::new();
    stdin.write(&original_asset_id);
    stdin.write(&encrypted_blob_id);
    stdin.write(&c_key_bytes);
    stdin.write(&aux_data.to_vec());
    stdin.write(&ctx.asset_encryption_key);

    // 生成 RSLH-VE 协议所需的诚实性采样证明
    let mut seed_h = Sha256::new();
    seed_h.update(&original_asset_id);
    seed_h.update(&encrypted_blob_id);
    seed_h.update(&c_key_bytes);
    let seed = seed_h.finalize();

    for i in 0..DEFAULT_SAMPLE_COUNT {
        let mut h = Sha256::new();
        h.update(&seed);
        h.update(&(i as u32).to_le_bytes());
        let idx = u32::from_le_bytes(h.finalize()[0..4].try_into().unwrap()) % 1000;

        let proof = create_honest_proof(
            &ctx.asset_encryption_key,
            &nonce,
            idx,
            &origin_data,
            &cipher_data,
        );
        stdin.write(&proof.global_index);
        stdin.write(&proof.origin_shard);
        stdin.write(&proof.cipher_shard);
    }

    // 3. === 设置并运行 Prover ===
    env::set_var("NETWORK_PRIVATE_KEY", env::var("SP1_PRIVATE_KEY").unwrap());
    let client = ProverClient::builder()
        .network_for(NetworkMode::Mainnet)
        .build()
        .await;
    let pk = client.setup(VDD_ELF).await?;
    let vk_string = pk.verifying_key().bytes32().to_string();
    println!(">>> Submitting VDD proof generation request to network...");
    let proof = client.prove(&pk, stdin).compressed().groth16().await?;
    println!(">>> VDD proof generated by network.");

    let public_values = proof.public_values.to_vec().into();
    let proof_bytes = proof.bytes();

    println!(">>> [VDD PROOF] zkVM Outputs:");
    println!("  - VK: {}", vk_string);
    println!(
        "  - Public Values: 0x{}",
        hex::encode(&proof.public_values.to_vec())
    );
    println!("  - Proof Length: {} bytes", proof_bytes.len());

    Ok((proof_bytes.into(), public_values, vk_string))
}

/// # [PROOF] 生成 VSS (Verifiable Secret Sharing) 证明
///
/// ## 角色
/// 卖家 (证明者)
///
/// ## 作用
/// 使用 SP1 ZKVM 生成一个 ZK 证明，公开证明一个封装后的数据密钥 (`wrapped_asset_key_vec`) 是由一个公开的共享密钥 (`secret_sharing_key`)
/// 正确加密一个私密的数据密钥 (`asset_encryption_key`) 得来的。这确保了卖家没有用错误的密钥进行封装。
///
/// ## 流程
/// 1. **构建输入 (Stdin)**: 按照 Guest 程序 (`vss-program`) 的要求，将共享密钥、数据密钥和 nonce 等写入 `SP1Stdin`。
/// 2. **调用 Prover**:
///    - `client.setup(VSS_ELF)`: 加载 VSS Guest 程序的 ELF 文件。
///    - `client.prove(...)`: 执行证明过程，生成 ZK 证明。
///
/// ## 输出
/// - `(Bytes, Bytes)`: ZK 证明和公开值。
pub async fn generate_vss_proof(v_k: [u8; 32], d_k: [u8; 32]) -> Result<(Bytes, Bytes, String)> {
    println!(">>> [VSS PROOF] zkVM Inputs:");
    println!("  - d_k (asset_encryption_key): 0x{}", hex::encode(d_k));
    println!("  - v_k (secret_sharing_key): 0x{}", hex::encode(v_k));
    println!("  - nonce: 0x{}", hex::encode(vec![0u8; 12]));

    let mut stdin = SP1Stdin::new();
    stdin.write(&1u8); // length
    stdin.write(&d_k.to_vec()); // message, matches guest read::<Vec<u8>>()
    stdin.write(&v_k); // watcher's key, matches guest read::<[u8; 32]>()
    stdin.write(&[0u8; 12]); // nonce, matches guest read::<[u8; 12]>()

    env::set_var("NETWORK_PRIVATE_KEY", env::var("SP1_PRIVATE_KEY").unwrap());
    let client = ProverClient::builder()
        .network_for(NetworkMode::Mainnet)
        .build()
        .await;
    let pk = client.setup(VSS_ELF).await?;
    let vk_string = pk.verifying_key().bytes32().to_string();
    println!(">>> Submitting VSS proof generation request to network...");
    let proof = client.prove(&pk, stdin).compressed().groth16().await?;
    println!(">>> VSS proof generated by network.");

    let public_values = proof.public_values.to_vec().into();
    let proof_bytes = proof.bytes();

    println!(">>> [VSS PROOF] zkVM Outputs:");
    println!("  - VK: {}", vk_string);
    println!(
        "  - Public Values: 0x{}",
        hex::encode(&proof.public_values.to_vec())
    );
    println!("  - Proof Length: {} bytes", proof_bytes.len());

    Ok((proof_bytes.into(), public_values, vk_string))
}

async fn download_walrus_blob_with_backoff(
    walrus_client: &WalrusClient,
    walrus_blob_id: &str,
) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>> {
    let max_attempts = env::var("DROP_WALRUS_DOWNLOAD_ATTEMPTS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(8);
    let mut delay_secs = env::var("DROP_WALRUS_DOWNLOAD_INITIAL_DELAY_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(15);

    for attempt in 1..=max_attempts {
        match walrus_client
            .download_blob(&BlobId(walrus_blob_id.to_string()))
            .await
        {
            Ok(reader) => return Ok(reader),
            Err(error) if attempt < max_attempts => {
                println!(
                    ">>> [WALRUS] blob not retrievable yet (attempt {attempt}/{max_attempts}): {error}"
                );
                println!(">>> [WALRUS] retrying in {delay_secs}s...");
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                delay_secs = (delay_secs * 2).min(120);
            }
            Err(error) => {
                return Err(anyhow!(
                    "Walrus blob remained unavailable after {max_attempts} attempts: {error}"
                ));
            }
        }
    }

    Err(anyhow!("unreachable Walrus retry state"))
}

/// # [STAGE 4] 买家恢复数据 (Recovery)
///
/// ## 角色
/// 买家
///
/// ## 流程
/// 1. **监听事件**: 从 Hub 合约的 `DataKeyShared` 事件中获取所有买家对应的加密数据密钥列表。
/// 2. **定位并解密**: 找到属于自己的那份加密数据密钥，并使用之前保存的 `secret_sharing_key` 对其解密，从而得到 `asset_encryption_key`。
/// 3. **下载加密数据**: 使用 `walrus_blob_id` 从 Walrus 网络下载加密的资产文件。
/// 4. **解密资产**: 使用恢复出的 `asset_encryption_key` 对下载的加密文件进行解密，得到原始资产。
/// 5. **保存文件**: 将解密后的数据写入本地文件。
pub async fn stage_4_recovery(
    walrus: &WalrusClient,
    ctx: &BuyerContext,
    channel_address: Address,
    fulfill_tx_hash: H256,
    blob_id: String,
    secret_sharing_key: [u8; 32],
    original_len: usize,
) -> Result<()> {
    println!(">>> [STAGE 4] RECOVERY...");
    let channel_inst =
        channel_abi::ExchangeChannelContract::new(channel_address, ctx.signer.clone());
    let receipt = ctx
        .signer
        .get_transaction_receipt(fulfill_tx_hash)
        .await?
        .ok_or(anyhow!("fulfill receipt not found: {fulfill_tx_hash:#x}"))?;
    let event_topic = H256::from_slice(
        &ethers::utils::keccak256("DataKeyShared(address[],bytes32[])")[..],
    );
    let log = receipt
        .logs
        .into_iter()
        .find(|log| {
            log.address == channel_address
                && log
                    .topics
                    .first()
                    .map(|topic| *topic == event_topic)
                    .unwrap_or(false)
        })
        .ok_or(anyhow!(
            "No DataKeyShared log found in fulfill tx {fulfill_tx_hash:#x}"
        ))?;
    let shared_ev =
        channel_inst.decode_event::<DataKeySharedFilter>("DataKeyShared", log.topics, log.data)?;

    let pos = shared_ev
        .audiences
        .iter()
        .position(|&a| a == ctx.signer.address())
        .ok_or(anyhow!("Buyer address not found in DataKeyShared event"))?;
    let asset_key_vec = chacha8_decrypt(
        &shared_ev.encrypted_data_keys[pos].to_vec(),
        &secret_sharing_key,
        &[0u8; 12],
        0,
    )?;
    let asset_key: [u8; 32] = asset_key_vec
        .try_into()
        .map_err(|_| anyhow!("Recovered asset key must be exactly 32 bytes"))?;

    let mut reader = walrus.download_blob(&BlobId(blob_id)).await?;
    let mut ciphertext = Vec::new();
    reader.read_to_end(&mut ciphertext).await?;

    let nonce = derive_rslh_nonce(&asset_key, b"maenad_v1");
    let mut recovered_data = chacha8_decrypt(&ciphertext, &asset_key, &nonce, 0)?;
    recovered_data.truncate(original_len);
    fs::write(RECOVERED_ASSET_NAME, recovered_data)?;
    println!(">>> Asset recovered and saved to {}", RECOVERED_ASSET_NAME);
    Ok(())
}

/// # [MONITOR] 等待 Oracle 信号
///
/// ## 角色
/// 任何人 (通常是卖家，为了进入 Settle 阶段)
///
/// ## 作用
/// 轮询通道合约的 `oracleSuccessUntil` 映射，等待外部的 Oracle 服务对 Walrus 上的数据可用性进行验证。
/// 当 Oracle 确认数据可访问后，会更新这个映射中的时间戳。这是进入最终结算 (`settle`) 阶段的前提条件。
pub async fn wait_for_oracle_signal(
    channel_address: Address,
    encrypted_blob_id: [u8; 32],
    signer: Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
) -> Result<()> {
    println!(">>> [MONITOR] WAITING FOR ORACLE PULSE...");
    let channel_contract =
        channel_abi::ExchangeChannelContract::new(channel_address, signer.clone());
    let blob_id_vec = encrypted_blob_id.to_vec();
    let timeout_secs = env::var("DROP_ORACLE_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(ORACLE_TIMEOUT_SECS);
    let started_at = tokio::time::Instant::now();

    loop {
        let success_until = channel_contract
            .oracle_success_until(blob_id_vec.clone().into())
            .call()
            .await?;
        let now = U256::from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs());
        if success_until > now {
            println!(
                ">>> [SIGNAL] ORACLE VERIFIED. SUCCESS UNTIL: {}",
                success_until
            );
            break;
        }
        if started_at.elapsed() > Duration::from_secs(timeout_secs) {
            return Err(anyhow!(
                "Oracle pulse timeout after {} seconds",
                timeout_secs
            ));
        }
        println!(">>> [WAIT] Oracle pulse not found. Retrying in 15s...");
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
    }
    Ok(())
}

/// # [STAGE 5] 卖家结算 (Settle)
///
/// ## 角色
/// 卖家
///
/// ## 作用
/// 在 Oracle 确认数据可用性后，调用 `settle` 方法，完成交易的最后一步。
/// 合约会验证 Oracle 信号，并将买家支付的款项转给卖家。
pub async fn stage_5_settle(
    ctx: &SellerContext,
    channel_address: Address,
    buyer: Address,
    info: channel_abi::ExchangeInfo,
    data_ver: [u8; 32],
    encrypted_blob_id: [u8; 32],
) -> Result<()> {
    println!(">>> [STAGE 5] SETTLEMENT...");
    let channel_contract =
        channel_abi::ExchangeChannelContract::new(channel_address, ctx.signer.clone());

    let arg_ver = data_ver.into();
    let arg_cipher = encrypted_blob_id.to_vec().into();

    channel_contract
        .settle(buyer, info, arg_ver, arg_cipher)
        .send()
        .await?
        .await?;
    Ok(())
}

// ==========================================================
// --- [Main 运行闭环] ---
// ==========================================================

pub struct SellerContext {
    pub signer: Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
    pub owner_sk_bytes: [u8; 32],
    pub asset_encryption_key: [u8; 32],
}
pub struct BuyerContext {
    pub signer: Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 在执行主流程前，先进行全面的配置和环境检查
    dotenv().ok();
    config_check::run_config_checks().await?;

    let provider = Provider::<Http>::try_from(configured_rpc_url())?;
    let walrus_endpoint = configured_walrus_endpoint();
    let walrus_config = WalrusConfig {
        aggregator_url: walrus_endpoint.clone(),
        publisher_url: walrus_endpoint,
        api_key: "".into(),
        blockberry_base: "".into(),
        send_object_to: None,
    };
    let walrus_client = WalrusClient::new(walrus_config);

    // 初始化买家和卖家的上下文，包含钱包签名器和密钥
    let seller_wallet = env::var("SELLER_KEY")?
        .parse::<LocalWallet>()?
        .with_chain_id(ARBITRUM_SEPOLIA_CHAIN_ID);
    let buyer_wallet = env::var("BUYER_KEY")?
        .parse::<LocalWallet>()?
        .with_chain_id(ARBITRUM_SEPOLIA_CHAIN_ID);

    let seller_ctx = SellerContext {
        signer: Arc::new(SignerMiddleware::new(provider.clone(), seller_wallet)),
        owner_sk_bytes: [0x11; 32],
        asset_encryption_key: [0x22; 32],
    };
    let buyer_ctx = BuyerContext {
        signer: Arc::new(SignerMiddleware::new(provider.clone(), buyer_wallet)),
    };

    // --- 执行端到端完整流程 ---

    // 1. 卖家挂牌
    let listing = stage_1_listing(&walrus_client, &seller_ctx).await?;

    // 1.5. 卖家提交数据密钥承诺
    stage_1_5_submit_key_commitment(&seller_ctx, listing.channel_address).await?;

    // 1.6. VDD 与 buyer 无关，提前提交并触发 Oracle。
    let vdd_tx_hash = stage_1_6_submit_vdd_proof(&walrus_client, &listing, &seller_ctx).await?;
    if vdd_tx_hash != H256::zero() {
        trigger_centralized_oracle_worker_if_enabled(vdd_tx_hash).await?;
    }
    wait_for_oracle_signal(
        listing.channel_address,
        listing.encrypted_blob_id,
        seller_ctx.signer.clone(),
    )
    .await?;

    // 2. 买家购买
    let seller_vss_pubkey = seller_public_key_bytes(&seller_ctx.owner_sk_bytes)?;
    let purchase = stage_2_purchase(
        &buyer_ctx,
        listing.unique_sale_id,
        listing.onchain_data_version,
        listing.channel_address,
        listing.original_asset_id,
        &seller_vss_pubkey,
    )
    .await?;

    // 3. 卖家履行
    let fulfill_tx_hash = stage_3_fulfill(&walrus_client, &listing, &purchase, &seller_ctx).await?;
    println!(">>> fulfill txHash: {:#x}", fulfill_tx_hash);

    // 4. 确认 Oracle 信号仍然有效
    wait_for_oracle_signal(
        listing.channel_address,
        listing.encrypted_blob_id,
        seller_ctx.signer.clone(),
    )
    .await?;

    // 5. 卖家结算，收取款项
    let (buyer_address, exchange_info) = get_purchase_info_from_event(
        provider.provider(),
        purchase.transaction_hash,
        listing.channel_address,
        listing.unique_sale_id,
    )
    .await?;
    stage_5_settle(
        &seller_ctx,
        listing.channel_address,
        buyer_address,
        exchange_info,
        listing.onchain_data_version,
        listing.encrypted_blob_id,
    )
    .await?;

    // 6. 买家恢复数据
    stage_4_recovery(
        &walrus_client,
        &buyer_ctx,
        listing.channel_address,
        fulfill_tx_hash,
        listing.walrus_blob_id,
        purchase.secret_sharing_key,
        listing.original_len,
    )
    .await?;

    println!("\n>>> End-to-end process completed successfully!");
    Ok(())
}
