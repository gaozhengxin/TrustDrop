use anyhow::{Result, anyhow};
use ethers::prelude::*;
use std::{sync::Arc, fs, env, time::{SystemTime, UNIX_EPOCH}};
use tokio::io::AsyncReadExt;

// 物理导入 Trait：启用 WalrusClient 异步流读取
use storage::{WalrusClient, BlobId, StorageNetwork, WalrusConfig};

use maenad_lib::kdf::key_derive;
use maenad_lib::ecies;

// 重构后的内部模块引用
use maenad_sdk::chacha8::{chacha8_encrypt, chacha8_decrypt};
// use maenad_sdk::proof::{run_vdd_proof};
use maenad_sdk::walrus::{compute_rs_id, upload_data_idempotent};
use sp1_sdk::{ProverClient, SP1Stdin};
use sha2::{Sha256, Digest};
use maenad_lib::rslh_ve::{create_honest_proof, derive_rslh_nonce, DEFAULT_SAMPLE_COUNT};

// ABI 引用
use maenad_sdk::abi::exchange_hub_contract as hub_abi;
use maenad_sdk::abi::exchange_channel_contract as channel_abi;
use maenad_sdk::abi::{DataKeySharedFilter, ExchangeChannelCreatedFilter, PurchaseEventFilter};

const VSS_ELF: &[u8] = include_bytes!("../../guest/vss/target/elf-compilation/riscv32im-succinct-zkvm-elf/release/vss-program");
const VDD_ELF: &[u8] = include_bytes!("../../guest/vdd/target/elf-compilation/riscv32im-succinct-zkvm-elf/release/program-vdd-walrus-rslhve");

// --- [物理常量定义] ---
pub const INPUT_ASSET_NAME: &str = "Mo.mp4";
pub const RECOVERED_ASSET_NAME: &str = "Mo_recovered.mp4";

pub const ARBITRUM_SEPOLIA_RPC: &str = "https://sepolia-rollup.arbitrum.io/rpc";
pub const WALRUS_LOCAL_ENDPOINT: &str = "http://localhost:31415"; 

pub const HUB_ADDRESS: &str = "0x2F0E2DeA5385e8Ea5234ea5c1f46A255fC330b5F";
pub const ARBITRUM_SEPOLIA_CHAIN_ID: u64 = 421614;
pub const LIVING_WINDOW_SECS: u64 = 7 * 24 * 3600; 

// ==========================================================
// --- [第一部分：原子工具函数] ---
// ==========================================================

pub fn compute_sale_id(channel_address: Address, chain_id: u64, nonce: U256) -> [u8; 32] {
    let mut packed_bytes = Vec::new();
    packed_bytes.extend_from_slice(channel_address.as_bytes());
    packed_bytes.extend_from_slice(&(chain_id as u32).to_be_bytes()); 
    let mut nonce_bytes = [0u8; 32]; 
    nonce.to_big_endian(&mut nonce_bytes);
    packed_bytes.extend_from_slice(&nonce_bytes);
    ethers::utils::keccak256(packed_bytes).into()
}

pub async fn get_or_create_channel(signer: Arc<SignerMiddleware<Provider<Http>, LocalWallet>>) -> Result<Address> {
    let hub_addr = HUB_ADDRESS.parse::<Address>().map_err(|_| anyhow!("Invalid HUB_ADDRESS"))?;
    let hub_contract = hub_abi::ExchangeHubContract::new(hub_addr, signer.clone());
    let initial_vss_pubkey = hub_abi::Pubkey { data: vec![0u8; 32].into() }; 
    
    let receipt = hub_contract.create_exchange_channel(initial_vss_pubkey).send().await?.await?.ok_or(anyhow!("Hub fail"))?;
    let ev = hub_contract.decode_event::<ExchangeChannelCreatedFilter>("ExchangeChannelCreated", receipt.logs[0].topics.clone(), receipt.logs[0].data.clone())?;
    Ok(ev.channel)
}

pub async fn get_purchase_info_from_event(client: &Provider<Http>, transaction_hash: H256, channel_address: Address) -> Result<(Address, channel_abi::ExchangeInfo)> {
    let receipt = client.get_transaction_receipt(transaction_hash).await?.ok_or(anyhow!("TX missing"))?;
    let log = receipt.logs.iter().find(|l| l.address == channel_address).ok_or(anyhow!("No Log"))?;
    let channel_inst = channel_abi::ExchangeChannelContract::new(channel_address, Arc::new(client.clone()));
    let purchase_ev = channel_inst.decode_event::<PurchaseEventFilter>("PurchaseEvent", log.topics.clone(), log.data.clone())?;
    
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

// ==========================================================
// --- [第二部分：协议阶段实现 - 语义化重构] ---
// ==========================================================

/// STAGE 1: Listing
pub async fn stage_1_listing(walrus: &WalrusClient, ctx: &SellerContext) -> Result<([u8; 32], [u8; 32], String, Address, [u8; 32], [u8; 32])> {
    println!(">>> [STAGE 1] LISTING...");
    let file_payload = fs::read(INPUT_ASSET_NAME)?;
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

/// STAGE 2: Purchase
pub async fn stage_2_purchase(ctx: &BuyerContext, unique_sale_id: [u8; 32], onchain_data_version: [u8; 32], channel_address: Address, original_asset_id: [u8; 32], seller_vss_pub: &[u8]) -> Result<([u8; 32], H256)> {
    println!(">>> [STAGE 2] PURCHASE...");
    let secret_sharing_key = key_derive(&[0xbb; 32], &original_asset_id).map_err(|e| anyhow!(e))?;
    let (encrypted_vss_key, eph_pk) = ecies::encrypt(seller_vss_pub, &secret_sharing_key)?;
    
    let arg_deadline = U256::from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + LIVING_WINDOW_SECS + 86400);
    let arg_price = 10u128.pow(16).into();
    let arg_vss_commit = blake3::hash(&secret_sharing_key).into();

    let channel_contract = channel_abi::ExchangeChannelContract::new(channel_address, ctx.signer.clone());
    let tx = channel_contract.purchase(unique_sale_id, onchain_data_version.into(), arg_price, arg_deadline, eph_pk.into(), arg_vss_commit, encrypted_vss_key)
        .value(arg_price).send().await?.await?;
    Ok((secret_sharing_key, tx.unwrap().transaction_hash))
}

/// STAGE 3: Fulfill
pub async fn stage_3_fulfill(walrus_client: &WalrusClient, walrus_blob_id: &str, ctx: &SellerContext, channel_address: Address, purchase_tx_hash: H256, original_asset_id: [u8; 32], encrypted_blob_id: [u8; 32]) -> Result<()> {
    println!(">>> [STAGE 3] FULFILL...");
    let (buyer, exchange_info) = get_purchase_info_from_event(ctx.signer.provider(), purchase_tx_hash, channel_address).await?;
    
    let channel_contract = channel_abi::ExchangeChannelContract::new(channel_address, ctx.signer.clone());
    let buyer_idx = channel_contract.audience_index(buyer).call().await?;
    let (_, encrypted_vss_from_chain) = channel_contract.audience_list(buyer_idx).call().await?;
    
    let secret_sharing_key = ecies::decrypt(&ctx.owner_sk_bytes, &encrypted_vss_from_chain, &exchange_info.data_commitment.to_vec())?;
    let wrapped_asset_key_vec = chacha8_encrypt(&ctx.asset_encryption_key.to_vec(), &secret_sharing_key, &[0u8; 12], 0)?;
    
    let (v_proof, v_pv) = generate_vss_proof(secret_sharing_key, ctx.asset_encryption_key).await?;
    let (d_proof, d_pv) = generate_vdd_proof(walrus_client, walrus_blob_id, ctx, original_asset_id, encrypted_blob_id).await?;

    let arg_vss = channel_abi::Vssargs { 
        encrypted_data_key: wrapped_asset_key_vec.try_into().unwrap(), 
        proof: v_proof, public_values: v_pv 
    };
    let arg_vdd = channel_abi::Vddargs { 
        proof: d_proof, public_values: d_pv, c_cipher: encrypted_blob_id.to_vec().into() 
    };
    let arg_ver = ethers::utils::keccak256(original_asset_id).into();

    channel_contract.fulfill(buyer, exchange_info, arg_ver, arg_vss, arg_vdd).send().await?.await?;
    Ok(())
}

pub async fn generate_vdd_proof(walrus_client: &WalrusClient, walrus_blob_id: &str, ctx: &SellerContext, original_asset_id: [u8; 32], encrypted_blob_id: [u8; 32]) -> Result<(Bytes, Bytes)> {
    // 1. === Prepare inputs on host ===
    let origin_data = fs::read(INPUT_ASSET_NAME)?;

    let mut reader = walrus_client.download_blob(&BlobId(walrus_blob_id.to_string())).await?;
    let mut cipher_data = Vec::new();
    reader.read_to_end(&mut cipher_data).await?;

    let c_key_bytes: [u8; 32] = Sha256::digest(&ctx.asset_encryption_key).into();
    let aux_data = b"maenad_v1";
    let nonce = derive_rslh_nonce(&ctx.asset_encryption_key, aux_data);

    // 2. === Build SP1 stdin ===
    let mut stdin = SP1Stdin::new();
    stdin.write(&original_asset_id);
    stdin.write(&encrypted_blob_id);
    stdin.write(&c_key_bytes);
    stdin.write(&aux_data.to_vec());
    stdin.write(&ctx.asset_encryption_key);

    // Generate sampling proofs
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
        stdin.write_vec(proof.origin_shard);
        stdin.write_vec(proof.cipher_shard);
    }

    // 3. === Setup & Prove ===
    let client = ProverClient::from_env();
    let (pk, _) = client.setup(VDD_ELF);
    let proof = client.prove(&pk, &stdin).plonk().run().expect("proving failed");

    let public_values = proof.public_values.to_vec().into();
    let proof_bytes = proof.bytes();

    Ok((proof_bytes.into(), public_values))
}


pub async fn generate_vss_proof(v_k: [u8; 32], d_k: [u8; 32]) -> Result<(Bytes, Bytes)> {
    let mut stdin = SP1Stdin::new();
    stdin.write(&1u8);
    stdin.write_vec(d_k.to_vec());
    stdin.write_vec(v_k.to_vec());
    stdin.write_vec(vec![0u8; 12]);

    let client = ProverClient::from_env();
    let (pk, _) = client.setup(VSS_ELF);
    let proof = client.prove(&pk, &stdin).plonk().run().expect("proving failed");

    let public_values = proof.public_values.to_vec().into();
    let proof_bytes = proof.bytes();

    Ok((proof_bytes.into(), public_values))
}


/// STAGE 4: Recovery
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
    Ok(())
}

/// 核心修正：监听 Oracle 成功信号 (物理探测从 0 变为非 0)
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

/// STAGE 5: Settle
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
    let provider = Provider::<Http>::try_from(ARBITRUM_SEPOLIA_RPC)?;
    let walrus_config = WalrusConfig {
        aggregator_url: WALRUS_LOCAL_ENDPOINT.to_string(),
        publisher_url: WALRUS_LOCAL_ENDPOINT.to_string(),
        api_key: "".into(), blockberry_base: "".into(), send_object_to: None,
    };
    let walrus_client = WalrusClient::new(walrus_config);

    let seller_ctx = SellerContext {
        signer: Arc::new(SignerMiddleware::new(provider.clone(), env::var("SELLER_KEY")?.parse()?)),
        owner_sk_bytes: [0x11; 32], asset_encryption_key: [0x22; 32]
    };
    let buyer_ctx = BuyerContext {
        signer: Arc::new(SignerMiddleware::new(provider.clone(), env::var("BUYER_KEY")?.parse()?))
    };

    // 1. 挂牌
    let (unique_sale_id, onchain_data_version, walrus_blob_id, channel_address, original_asset_id, encrypted_blob_id) = stage_1_listing(&walrus_client, &seller_ctx).await?;
    
    // 2. 支付
    let (secret_sharing_key, purchase_transaction_hash) = stage_2_purchase(&buyer_ctx, unique_sale_id, onchain_data_version, channel_address, original_asset_id, &[0x02; 33]).await?;
    
    // 3. 履行
    stage_3_fulfill(&walrus_client, &walrus_blob_id, &seller_ctx, channel_address, purchase_transaction_hash, original_asset_id, encrypted_blob_id).await?;
    
    // 4. 等待 Oracle 确认 (物理心跳)
    wait_for_oracle_signal(channel_address, encrypted_blob_id, seller_ctx.signer.clone()).await?;

    // 5. 结算
    let (buyer_address, exchange_info) = get_purchase_info_from_event(provider.provider(), purchase_transaction_hash, channel_address).await?;
    stage_5_settle(&seller_ctx, channel_address, buyer_address, exchange_info, onchain_data_version, encrypted_blob_id).await?;

    // 6. 买家数据恢复
    stage_4_recovery(&walrus_client, &buyer_ctx, walrus_blob_id, secret_sharing_key).await?;

    Ok(())
}
