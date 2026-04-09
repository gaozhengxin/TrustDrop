use anyhow::{Result, anyhow};
use ethers::prelude::*;
use std::{sync::Arc, fs, env, time::{SystemTime, UNIX_EPOCH}};
use tokio::io::AsyncReadExt;

// 物理导入 Trait：启用 WalrusClient 异步流读取
use storage::{WalrusClient, BlobId, StorageNetwork, WalrusConfig};

use maenad_lib::kdf::key_derive;
use maenad_lib::ecies;
use dotenv::dotenv;

mod config_check;

// 重构后的内部模块引用
use maenad_sdk::chacha8::{chacha8_encrypt, chacha8_decrypt};
// use maenad_sdk::proof::{run_vdd_proof};
use maenad_sdk::walrus::{compute_rs_id, upload_data_idempotent};
use sp1_sdk::{network::{FulfillmentStrategy, NetworkMode}, Prover, ProverClient, SP1Stdin, HashableKey};
use sha2::{Sha256, Digest};
use maenad_lib::rslh_ve::{create_honest_proof, derive_rslh_nonce, DEFAULT_SAMPLE_COUNT, SYMBOL_SIZE};

// ABI 引用
use maenad_sdk::abi::exchange_hub_contract as hub_abi;
use maenad_sdk::abi::exchange_channel_contract as channel_abi;

abigen!(VSSVerifierContract, r#"[
    function verifyVSSProof(bytes calldata _publicValues, bytes calldata _proofBytes) external view
]"#);

abigen!(VDDVerifierContract, r#"[
    function verifyVDDProof(bytes calldata _publicValues, bytes calldata _proofBytes) external view
]"#);

use maenad_sdk::abi::{DataKeySharedFilter, ExchangeChannelCreatedFilter};

const VSS_ELF: &[u8] = include_bytes!("../../guest/vss/target/elf-compilation/riscv32im-succinct-zkvm-elf/release/vss-program");
const VDD_ELF: &[u8] = include_bytes!("../../guest/vdd/target/elf-compilation/riscv32im-succinct-zkvm-elf/release/program-vdd-walrus-rslhve");

// --- [物理常量定义] ---
pub const INPUT_ASSET_NAME: &str = "Mo.mp4";
pub const RECOVERED_ASSET_NAME: &str = "Mo_recovered.mp4";

pub const ARBITRUM_SEPOLIA_RPC: &str = "https://sepolia-rollup.arbitrum.io/rpc";
pub const WALRUS_LOCAL_ENDPOINT: &str = "http://localhost:31415"; 

pub const HUB_ADDRESS: &str = "0x2e506eF3F3cE222F276ddA64Df239CEF92683a78";
pub const ARBITRUM_SEPOLIA_CHAIN_ID: u64 = 421614;
pub const LIVING_WINDOW_SECS: u64 = 7 * 24 * 3600; 

pub const VSS_VERIFIER_ADDRESS: &str = "0x5e80ed679fb9f4050a5c7ede5ccbe39178f142a2";
pub const VDD_VERIFIER_ADDRESS: &str = "0x154D59Ed30B7784B5c9324b32b9ec5d6c8DE4071";

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
pub async fn get_or_create_channel(signer: Arc<SignerMiddleware<Provider<Http>, LocalWallet>>) -> Result<Address> {
    let hub_addr = HUB_ADDRESS.parse::<Address>().map_err(|_| anyhow!("Invalid HUB_ADDRESS"))?;
    let hub_contract = hub_abi::ExchangeHubContract::new(hub_addr, signer.clone());
    let initial_vss_pubkey = hub_abi::Pubkey { data: vec![0u8; 32].into() }; 
    
    let receipt = hub_contract.create_exchange_channel(initial_vss_pubkey).send().await?.await?.ok_or(anyhow!("Hub fail"))?;
    let ev = hub_contract.decode_event::<ExchangeChannelCreatedFilter>("ExchangeChannelCreated", receipt.logs[0].topics.clone(), receipt.logs[0].data.clone())?;
    Ok(ev.channel)
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
pub async fn get_purchase_info_from_event(client: &Provider<Http>, transaction_hash: H256, _channel_address: Address) -> Result<(Address, channel_abi::ExchangeInfo)> {
    let hub_addr = HUB_ADDRESS.parse::<Address>()?;
    let receipt = client.get_transaction_receipt(transaction_hash).await?.ok_or(anyhow!("TX missing"))?;
    
    let log = receipt.logs.iter().find(|l| l.address == hub_addr).ok_or(anyhow!("No PurchaseEvent Log Found from Hub"))?;

    let hub_inst = hub_abi::ExchangeHubContract::new(hub_addr, Arc::new(client.clone()));
    let purchase_ev = hub_inst.decode_event::<hub_abi::PurchaseEventFilter>("PurchaseEvent", log.topics.clone(), log.data.clone())?;
    
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
    public_values: Bytes,
    proof_bytes: Bytes,
) -> Result<()> {
    println!(">>> [SIMULATION] Testing VSS verification on Verifier: {}", verifier_address);
    println!("  - Input VK: {}", vk_string);
    println!("  - Input Public Values: 0x{}", hex::encode(public_values.to_vec()));
    println!("  - Input Proof (len): {} bytes", proof_bytes.len());
    println!("  - Input Proof: 0x{}", hex::encode(proof_bytes.to_vec()));

    let contract = VSSVerifierContract::new(verifier_address, Arc::new(provider.clone()));
    let call_builder = contract.verify_vss_proof(public_values.clone(), proof_bytes.clone());
    
    if let Some(calldata) = call_builder.calldata() {
        println!(">>> [DEBUG] VSS Transaction Calldata: 0x{}", hex::encode(&calldata));
    }

    match call_builder.call().await {
        Ok(_) => {
            println!(">>> [SIMULATION] VSS Proof verified successfully!");
            Ok(())
        }
        Err(e) => {
            if let Some(revert_data) = e.as_revert() {
                let hex_revert = hex::encode(revert_data);
                println!(">>> [DEBUG] Raw Revert Data: 0x{}", hex_revert);
                let selector = if hex_revert.len() >= 8 { &hex_revert[0..8] } else { &hex_revert };
                let meaning = match selector {
                    "7fcdd1f4" => "InvalidPublicValues() - SP1 Verifier rejected the public values. This almost always means the VK hardcoded in your deployed verifier contract is stale and doesn't match the newly compiled guest program!",
                    "09bde339" => "InvalidProof() - The ZK proof itself failed mathematical verification.",
                    "1b50428d" => "WrongVerificationKey() - The VK does not match the one expected by the Verifier.",
                    _ => "Unknown Custom Error",
                };
                let err_msg = format!("VSS Proof FAILED. Details: Revert(0x{}) -> {}", selector, meaning);
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
    public_values: Bytes,
    proof_bytes: Bytes,
) -> Result<()> {
    println!(">>> [SIMULATION] Testing VDD verification on Verifier: {}", verifier_address);
    println!("  - Input VK: {}", vk_string);
    println!("  - Input Public Values: 0x{}", hex::encode(public_values.to_vec()));
    println!("  - Input Proof (len): {} bytes", proof_bytes.len());

    let contract = VDDVerifierContract::new(verifier_address, Arc::new(provider.clone()));
    let call_builder = contract.verify_vdd_proof(public_values.clone(), proof_bytes.clone());
    
    if let Some(calldata) = call_builder.calldata() {
        println!(">>> [DEBUG] VDD Transaction Calldata: 0x{}", hex::encode(&calldata));
    }

    match call_builder.call().await {
        Ok(_) => {
            println!(">>> [SIMULATION] VDD Proof verified successfully!");
            Ok(())
        }
        Err(e) => {
            if let Some(revert_data) = e.as_revert() {
                let hex_revert = hex::encode(revert_data);
                println!(">>> [DEBUG] Raw Revert Data: 0x{}", hex_revert);
                let selector = if hex_revert.len() >= 8 { &hex_revert[0..8] } else { &hex_revert };
                let meaning = match selector {
                    "7fcdd1f4" => "InvalidPublicValues() - SP1 Verifier rejected the public values. Check if your deployed verifier VK is out of date!",
                    "09bde339" => "InvalidProof() - The ZK proof itself failed mathematical verification.",
                    "1b50428d" => "WrongVerificationKey() - The VK does not match the one expected by the Verifier.",
                    _ => "Unknown Custom Error",
                };
                let err_msg = format!("VDD Proof FAILED. Details: Revert(0x{}) -> {}", selector, meaning);
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
pub async fn stage_1_listing(walrus: &WalrusClient, ctx: &SellerContext) -> Result<([u8; 32], [u8; 32], String, Address, [u8; 32], [u8; 32])> {
    println!(">>> [STAGE 1] LISTING...");
    let mut file_payload = fs::read(INPUT_ASSET_NAME)?;
    let original_len = file_payload.len();
    let padded_len = (original_len + SYMBOL_SIZE - 1) / SYMBOL_SIZE * SYMBOL_SIZE;
    file_payload.resize(padded_len, 0);

    let original_asset_id = compute_rs_id(&file_payload)?;
    
    let asset_nonce = derive_rslh_nonce(&ctx.asset_encryption_key, b"maenad_v1");
    let encrypted_asset_data = chacha8_encrypt(&file_payload, &ctx.asset_encryption_key, &asset_nonce, 0)?;
    let encrypted_blob_id = compute_rs_id(&encrypted_asset_data)?;
    let walrus_blob_id = upload_data_idempotent(walrus, encrypted_asset_data).await?;

    let channel_addr = get_or_create_channel(ctx.signer.clone()).await?;
    let channel_contract = channel_abi::ExchangeChannelContract::new(channel_addr, ctx.signer.clone());
    
    let sale_nonce = channel_contract.nonce().call().await?;
    let unique_sale_id = compute_sale_id(channel_addr, ARBITRUM_SEPOLIA_CHAIN_ID, sale_nonce);
    let onchain_data_version = ethers::utils::keccak256(original_asset_id);

    let arg_commit = channel_abi::DataCommitment { data: original_asset_id.to_vec().into() };
    let arg_price = 10u128.pow(16).into(); // 0.01 ETH
    let arg_meta = "Maenad Asset v1".to_string();

    channel_contract.list_file(arg_commit, arg_price, arg_meta).send().await?.await?;
    Ok((unique_sale_id, onchain_data_version.into(), walrus_blob_id, channel_addr, original_asset_id, encrypted_blob_id))
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
async fn stage_1_5_submit_key_commitment(ctx: &SellerContext, channel_address: Address) -> Result<()> {
    println!(">>> [STAGE 1.5] SUBMITTING DATA KEY COMMITMENT...");
    let channel_contract = channel_abi::ExchangeChannelContract::new(channel_address, ctx.signer.clone());
    
    let data_key_commitment: [u8; 32] = *blake3::hash(&ctx.asset_encryption_key).as_bytes();
    
    channel_contract
        .submit_data_key_commitment(data_key_commitment.into())
        .send()
        .await?
        .await?;
        
    println!(">>> Data key commitment submitted.");
    Ok(())
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
pub async fn stage_2_purchase(ctx: &BuyerContext, unique_sale_id: [u8; 32], onchain_data_version: [u8; 32], channel_address: Address, original_asset_id: [u8; 32], seller_vss_pub: &[u8]) -> Result<([u8; 32], H256)> {
    println!(">>> [STAGE 2] PURCHASE...");
    let secret_sharing_key = key_derive(&[0xbb; 32], &original_asset_id).map_err(|e| anyhow!(e))?;
    let (encrypted_vss_key, _eph_pk) = ecies::encrypt(seller_vss_pub, &secret_sharing_key)?;
    
    let arg_deadline = U256::from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + LIVING_WINDOW_SECS + 86400);
    let arg_price = 10u128.pow(16).into();
    let arg_vss_commit: [u8; 32] = *blake3::hash(&secret_sharing_key).as_bytes();

    let channel_contract = channel_abi::ExchangeChannelContract::new(channel_address, ctx.signer.clone());
    let mut encrypted_vss_key_bytes32 = [0u8; 32];
    if encrypted_vss_key.len() >= 32 {
        encrypted_vss_key_bytes32.copy_from_slice(&encrypted_vss_key[..32]);
    }
    
    let tx = channel_contract.purchase(
            unique_sale_id, 
            onchain_data_version.into(), 
            arg_price, 
            arg_deadline, 
            original_asset_id.to_vec().into(), // Correct dataCommitment
            arg_vss_commit.into(), 
            encrypted_vss_key_bytes32.into()
        )
        .value(arg_price).send().await?.await?;
    Ok((secret_sharing_key, tx.unwrap().transaction_hash))
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
pub async fn stage_3_fulfill(walrus_client: &WalrusClient, walrus_blob_id: &str, ctx: &SellerContext, channel_address: Address, purchase_tx_hash: H256, original_asset_id: [u8; 32], encrypted_blob_id: [u8; 32]) -> Result<()> {
    println!(">>> [STAGE 3] FULFILL...");
    let mut errors: Vec<anyhow::Error> = Vec::new();

    let channel_contract = channel_abi::ExchangeChannelContract::new(channel_address, ctx.signer.clone());
    let (buyer, exchange_info) = get_purchase_info_from_event(ctx.signer.provider(), purchase_tx_hash, channel_address).await?;
    
    let secret_sharing_key = key_derive(&[0xbb; 32], &original_asset_id).map_err(|e| anyhow!(e))?;
    let wrapped_asset_key_vec = chacha8_encrypt(&ctx.asset_encryption_key.to_vec(), &secret_sharing_key, &[0u8; 12], 0)?;

    // --- 使用硬编码的 Verifier 地址 ---
    let vss_verifier_addr = Some(VSS_VERIFIER_ADDRESS.parse::<Address>().unwrap());
    let vdd_verifier_addr = Some(VDD_VERIFIER_ADDRESS.parse::<Address>().unwrap());
    println!("  - Using VSS Verifier at: {}", VSS_VERIFIER_ADDRESS);
    println!("  - Using VDD Verifier at: {}", VDD_VERIFIER_ADDRESS);

    // --- 生成并独立模拟 VSS 证明 ---
    let vss_result = generate_vss_proof(secret_sharing_key, ctx.asset_encryption_key).await;
    let (v_proof, v_pv) = if let Ok((proof, pv, vk)) = vss_result {
        if let Some(addr) = vss_verifier_addr {
            if let Err(e) = simulate_vss_verify(ctx.signer.provider(), addr, vk, pv.clone(), proof.clone()).await {
                errors.push(e);
            }
        }
        (proof, pv)
    } else if let Err(e) = vss_result {
        errors.push(anyhow!("VSS proof generation failed: {:?}", e));
        (Bytes::new(), Bytes::new())
    } else { (Bytes::new(), Bytes::new()) };

    // --- 生成并独立模拟 VDD 证明 (无论 VSS 是否成功) ---
    let vdd_result = generate_vdd_proof(walrus_client, walrus_blob_id, ctx, original_asset_id, encrypted_blob_id).await;
    let (d_proof, d_pv) = if let Ok((proof, pv, vk)) = vdd_result {
        if let Some(addr) = vdd_verifier_addr {
            if let Err(e) = simulate_vdd_verify(ctx.signer.provider(), addr, vk, pv.clone(), proof.clone()).await {
                errors.push(e);
            }
        }
        (proof, pv)
    } else if let Err(e) = vdd_result {
        errors.push(anyhow!("VDD proof generation failed: {:?}", e));
        (Bytes::new(), Bytes::new())
    } else { (Bytes::new(), Bytes::new()) };

    // --- 统一检查所有错误 ---
    if !errors.is_empty() {
        let combined_error = errors.into_iter().map(|e| e.to_string()).collect::<Vec<String>>().join("\n- ");
        return Err(anyhow!("One or more verification steps failed:\n- {}", combined_error));
    }

    // --- 构造参数并发送最终交易 ---
    println!(">>> All simulations passed. Submitting fulfill transaction...");
    let arg_vss = channel_abi::Vssargs { encrypted_data_key: wrapped_asset_key_vec.try_into().unwrap(), proof: v_proof, public_values: v_pv };
    let arg_vdd = channel_abi::Vddargs { proof: d_proof, public_values: d_pv, c_cipher: encrypted_blob_id.to_vec().into() };
    let arg_ver = ethers::utils::keccak256(original_asset_id).into();

    println!(">>> [STAGE 3] FULFILL CONTRACT CALL ARGS:");
    println!("  - buyer: {:?}", buyer);
    println!("  - exchange_info.sale_digest: 0x{}", hex::encode(exchange_info.sale_digest));
    println!("  - arg_ver: 0x{}", hex::encode(arg_ver));
    println!("  - arg_vss.encrypted_data_key: 0x{}", hex::encode(&arg_vss.encrypted_data_key));
    println!("  - arg_vss.public_values: 0x{}", hex::encode(arg_vss.public_values.to_vec()));
    println!("  - arg_vdd.c_cipher: 0x{}", hex::encode(arg_vdd.c_cipher.to_vec()));
    println!("  - arg_vdd.public_values: 0x{}", hex::encode(arg_vdd.public_values.to_vec()));

    channel_contract.fulfill(buyer, exchange_info, arg_ver, arg_vss, arg_vdd).send().await?.await?;
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
pub async fn generate_vdd_proof(walrus_client: &WalrusClient, walrus_blob_id: &str, ctx: &SellerContext, original_asset_id: [u8; 32], encrypted_blob_id: [u8; 32]) -> Result<(Bytes, Bytes, String)> {
    // 1. === 准备 VDD 电路所需的全部输入 ===
    let mut origin_data = fs::read(INPUT_ASSET_NAME)?;
    let original_len = origin_data.len();
    let padded_len = (original_len + SYMBOL_SIZE - 1) / SYMBOL_SIZE * SYMBOL_SIZE;
    origin_data.resize(padded_len, 0);
    
    let mut reader = walrus_client.download_blob(&BlobId(walrus_blob_id.to_string())).await?;
    let mut cipher_data = Vec::new();
    reader.read_to_end(&mut cipher_data).await?;

    // 【重大修复】：必须使用和 stage_1_5_submit_key_commitment 相同的哈希算法
    let c_key_bytes: [u8; 32] = *blake3::hash(&ctx.asset_encryption_key).as_bytes();
    let aux_data = b"maenad_v1";
    let nonce = derive_rslh_nonce(&ctx.asset_encryption_key, aux_data);

    println!(">>> [VDD PROOF] zkVM Inputs:");
    println!("  - original_asset_id: 0x{}", hex::encode(original_asset_id));
    println!("  - encrypted_blob_id: 0x{}", hex::encode(encrypted_blob_id));
    println!("  - c_key_bytes: 0x{}", hex::encode(c_key_bytes));
    println!("  - aux_data: 0x{}", hex::encode(aux_data));
    println!("  - asset_encryption_key: 0x{}", hex::encode(ctx.asset_encryption_key));

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

        let proof = create_honest_proof(&ctx.asset_encryption_key, &nonce, idx, &origin_data, &cipher_data);
        stdin.write(&proof.global_index);
        stdin.write(&proof.origin_shard);
        stdin.write(&proof.cipher_shard);
    }

    // 3. === 设置并运行 Prover ===
    env::set_var("NETWORK_PRIVATE_KEY", env::var("SP1_PRIVATE_KEY").unwrap());
    let client = ProverClient::builder().network_for(NetworkMode::Mainnet).build();
    let (pk, vk) = client.setup(VDD_ELF);
    let vk_string = vk.bytes32().to_string();
    println!(">>> Submitting VDD proof generation request to network...");
    let proof = tokio::task::spawn_blocking(move || {
        client.prove(&pk, &stdin)
            .private()
            .compressed()
            .strategy(FulfillmentStrategy::Auction)
            .groth16()
            .run()
    }).await??;
    println!(">>> VDD proof generated by network.");

    let public_values = proof.public_values.to_vec().into();
    let proof_bytes = proof.bytes();

    println!(">>> [VSS PROOF] zkVM Outputs:");
    println!("  - VK: {}", vk_string);
    println!("  - Public Values: 0x{}", hex::encode(&proof.public_values.to_vec()));
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
    stdin.write_slice(&d_k); // message
    stdin.write_slice(&v_k); // watcher's key
    stdin.write_slice(&vec![0u8; 12]); // nonce

    env::set_var("NETWORK_PRIVATE_KEY", env::var("SP1_PRIVATE_KEY").unwrap());
    let client = ProverClient::builder().network_for(NetworkMode::Mainnet).build();
    let (pk, vk) = client.setup(VSS_ELF);
    let vk_string = vk.bytes32().to_string();
    println!(">>> Submitting VSS proof generation request to network...");
    let proof = tokio::task::spawn_blocking(move || {
        client.prove(&pk, &stdin)
            .compressed()
            .strategy(FulfillmentStrategy::Auction)
            .groth16()
            .run()
    }).await??;
    println!(">>> VSS proof generated by network.");

    let public_values = proof.public_values.to_vec().into();
    let proof_bytes = proof.bytes();

    println!(">>> [VSS PROOF] zkVM Outputs:");
    println!("  - VK: {}", vk_string);
    println!("  - Public Values: 0x{}", hex::encode(&proof.public_values.to_vec()));
    println!("  - Proof Length: {} bytes", proof_bytes.len());

    Ok((proof_bytes.into(), public_values, vk_string))
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
pub async fn stage_4_recovery(walrus: &WalrusClient, ctx: &BuyerContext, blob_id: String, secret_sharing_key: [u8; 32]) -> Result<()> {
    println!(">>> [STAGE 4] RECOVERY...");
    let hub_inst = hub_abi::ExchangeHubContract::new(HUB_ADDRESS.parse::<Address>()?, ctx.signer.clone());
    let log = ctx.signer.get_logs(&Filter::new().address(vec![HUB_ADDRESS.parse::<Address>()?]).event("DataKeyShared(address[],bytes32[])")).await?.pop().ok_or(anyhow!("No key log"))?;
    let shared_ev = hub_inst.decode_event::<DataKeySharedFilter>("DataKeyShared", log.topics, log.data)?;
    
    let pos = shared_ev.audiences.iter().position(|&a| a == ctx.signer.address()).unwrap();
    let asset_key_vec = chacha8_decrypt(&shared_ev.encrypted_data_keys[pos].to_vec(), &secret_sharing_key, &[0u8; 12], 0)?;
    let asset_key: [u8; 32] = asset_key_vec.try_into().unwrap();

    let mut reader = walrus.download_blob(&BlobId(blob_id)).await?;
    let mut ciphertext = Vec::new(); reader.read_to_end(&mut ciphertext).await?; 
    
    let nonce = derive_rslh_nonce(&asset_key, b"maenad_v1");
    let recovered_data = chacha8_decrypt(&ciphertext, &asset_key, &nonce, 0)?;
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
pub async fn wait_for_oracle_signal(channel_address: Address, encrypted_blob_id: [u8; 32], signer: Arc<SignerMiddleware<Provider<Http>, LocalWallet>>) -> Result<()> {
    println!(">>> [MONITOR] WAITING FOR ORACLE PULSE...");
    let channel_contract = channel_abi::ExchangeChannelContract::new(channel_address, signer.clone());
    let blob_id_vec = encrypted_blob_id.to_vec();

    loop {
        let success_until = channel_contract.oracle_success_until(blob_id_vec.clone().into()).call().await?;
        if success_until > U256::zero() {
            println!(">>> [SIGNAL] ORACLE VERIFIED. SUCCESS UNTIL: {}", success_until);
            break;
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
pub async fn stage_5_settle(ctx: &SellerContext, channel_address: Address, buyer: Address, info: channel_abi::ExchangeInfo, data_ver: [u8; 32], encrypted_blob_id: [u8; 32]) -> Result<()> {
    println!(">>> [STAGE 5] SETTLEMENT...");
    let channel_contract = channel_abi::ExchangeChannelContract::new(channel_address, ctx.signer.clone());
    
    let arg_ver = data_ver.into();
    let arg_cipher = encrypted_blob_id.to_vec().into();

    channel_contract.settle(buyer, info, arg_ver, arg_cipher).send().await?.await?;
    Ok(())
}

// ==========================================================
// --- [Main 运行闭环] ---
// ==========================================================

pub struct SellerContext { pub signer: Arc<SignerMiddleware<Provider<Http>, LocalWallet>>, pub owner_sk_bytes: [u8; 32], pub asset_encryption_key: [u8; 32] }
pub struct BuyerContext { pub signer: Arc<SignerMiddleware<Provider<Http>, LocalWallet>> }

#[tokio::main]
async fn main() -> Result<()> {
    // 在执行主流程前，先进行全面的配置和环境检查
    dotenv().ok();
    config_check::run_config_checks().await?;

    let provider = Provider::<Http>::try_from(ARBITRUM_SEPOLIA_RPC)?;
    let walrus_config = WalrusConfig {
        aggregator_url: WALRUS_LOCAL_ENDPOINT.to_string(),
        publisher_url: WALRUS_LOCAL_ENDPOINT.to_string(),
        api_key: "".into(), blockberry_base: "".into(), send_object_to: None,
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
        owner_sk_bytes: [0x11; 32], asset_encryption_key: [0x22; 32]
    };
    let buyer_ctx = BuyerContext {
        signer: Arc::new(SignerMiddleware::new(provider.clone(), buyer_wallet))
    };

    // --- 执行端到端完整流程 ---
    
    // 1. 卖家挂牌
    let (unique_sale_id, onchain_data_version, walrus_blob_id, channel_address, original_asset_id, encrypted_blob_id) = stage_1_listing(&walrus_client, &seller_ctx).await?;

    // 1.5. 卖家提交数据密钥承诺
    stage_1_5_submit_key_commitment(&seller_ctx, channel_address).await?;
    
    // 2. 买家购买
    let (secret_sharing_key, purchase_transaction_hash) = stage_2_purchase(&buyer_ctx, unique_sale_id, onchain_data_version, channel_address, original_asset_id, &[0x02; 33]).await?;
    
    // 3. 卖家履行
    stage_3_fulfill(&walrus_client, &walrus_blob_id, &seller_ctx, channel_address, purchase_transaction_hash, original_asset_id, encrypted_blob_id).await?;
    
    // 4. 等待 Oracle 确认数据可用性
    wait_for_oracle_signal(channel_address, encrypted_blob_id, seller_ctx.signer.clone()).await?;

    // 5. 卖家结算，收取款项
    let (buyer_address, exchange_info) = get_purchase_info_from_event(provider.provider(), purchase_transaction_hash, channel_address).await?;
    stage_5_settle(&seller_ctx, channel_address, buyer_address, exchange_info, onchain_data_version, encrypted_blob_id).await?;

    // 6. 买家恢复数据
    stage_4_recovery(&walrus_client, &buyer_ctx, walrus_blob_id, secret_sharing_key).await?;

    println!("\n>>> End-to-end process completed successfully!");
    Ok(())
}
