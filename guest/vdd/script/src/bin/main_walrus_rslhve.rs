use chacha20::cipher::{KeyIvInit, StreamCipher};
use chacha20::{ChaCha8, Key, Nonce};
use clap::Parser;
use rand::{rng, RngCore};
use sha2::{Digest, Sha256};
use sp1_sdk::{include_elf, Elf, Prover, ProverClient, ProvingKey, SP1Stdin};

use drop_lib::rslh_ve::{
    create_honest_proof, derive_rslh_nonce, walrus_symbol_size, DEFAULT_SAMPLE_COUNT,
    MIN_VDD_BLOB_BYTES,
};
use drop_lib::walrus_address::compute_blob_id_default;
use drop_lib::walrus_open::{build_cipher_blob_opening, build_origin_blob_opening};

pub const VDD_WALRUS_RSLHVE_ELF: Elf = include_elf!("program-vdd-walrus-rslhve");

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    execute: bool,

    #[arg(long)]
    prove: bool,
}

fn bytes_to_hex(b: &[u8], len: usize) -> String {
    let take = b.len().min(len);
    hex::encode(&b[..take])
}

#[tokio::main]
async fn main() {
    sp1_sdk::utils::setup_logger();
    let args = Args::parse();

    if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    // 1. === Prepare inputs on host ===
    const DEFAULT_DATA_SIZE: usize = MIN_VDD_BLOB_BYTES as usize;
    let data_size = std::env::var("VDD_RSLHVE_DATA_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_DATA_SIZE);
    let mut origin_data = vec![0u8; data_size];
    rng().fill_bytes(&mut origin_data);
    if (origin_data.len() as u64) < MIN_VDD_BLOB_BYTES {
        eprintln!("error: VDD requires an asset of at least 1 MiB");
        std::process::exit(1);
    }

    let mut key = [0u8; 32];
    rng().fill_bytes(&mut key);

    // 计算核心承诺
    let c_origin = compute_blob_id_default(&origin_data).unwrap();
    let c_origin_bytes: [u8; 32] = (*c_origin.as_ref()).try_into().unwrap();

    let c_key_bytes: [u8; 32] = *blake3::hash(&key).as_bytes();

    let aux_data = b"trustdrop_asset_v1";
    let nonce = derive_rslh_nonce(&key, aux_data);
    let symbol_size = walrus_symbol_size(origin_data.len() as u64);

    // 线性加密生成密文
    let mut cipher_data = origin_data.clone();
    let mut cipher_stream = ChaCha8::new(Key::from_slice(&key), Nonce::from_slice(&nonce));
    cipher_stream.apply_keystream(&mut cipher_data);
    let c_cipher = compute_blob_id_default(&cipher_data).unwrap();
    let c_cipher_bytes: [u8; 32] = (*c_cipher.as_ref()).try_into().unwrap();

    // 构造预期的公共输出 (对齐 Guest 的 commit_slice)
    let mut combined_expected = Vec::with_capacity(96);
    combined_expected.extend_from_slice(&c_origin_bytes);
    combined_expected.extend_from_slice(&c_key_bytes);
    combined_expected.extend_from_slice(&c_cipher_bytes);

    println!("--- VDD Walrus RSLH-VE Preparation ---");
    println!("  c_origin: {}", hex::encode(&c_origin_bytes));
    println!("  c_key:    {}", hex::encode(&c_key_bytes));
    println!("  c_cipher: {}", hex::encode(&c_cipher_bytes));

    // 2. === Build SP1 stdin ===
    let mut stdin = SP1Stdin::new();
    stdin.write(&c_origin_bytes);
    stdin.write(&c_cipher_bytes);
    stdin.write(&c_key_bytes);
    stdin.write(&aux_data.to_vec());
    stdin.write(&key);

    // 基于承诺派生种子生成 Shards
    let mut seed_h = Sha256::new();
    seed_h.update(&c_origin_bytes);
    seed_h.update(&c_cipher_bytes);
    seed_h.update(&c_key_bytes);
    let seed: [u8; 32] = seed_h.finalize().into();

    for i in 0..DEFAULT_SAMPLE_COUNT {
        let mut h = Sha256::new();
        h.update(&seed);
        h.update(&(i as u32).to_le_bytes());
        let idx = u32::from_le_bytes(h.finalize()[0..4].try_into().unwrap()) % 1000;

        let proof = create_honest_proof(&key, &nonce, idx, symbol_size, &origin_data, &cipher_data);
        stdin.write(&proof.global_index);
        stdin.write(&proof.origin_shard);
        stdin.write(&proof.cipher_shard);
    }

    let origin_opening = build_origin_blob_opening(&origin_data, &seed, symbol_size)
        .expect("origin blob opening construction failed");
    assert_eq!(&origin_opening.blob_id[..], c_origin.as_ref());

    // Walrus 承诺打开（guest 现在要求该输入）
    let cipher_opening = build_cipher_blob_opening(&cipher_data, &seed, symbol_size)
        .expect("cipher blob opening construction failed");
    assert_eq!(
        &cipher_opening.blob_id[..],
        c_cipher.as_ref(),
        "opening blob id must match c_cipher"
    );
    stdin.write(&origin_opening);
    stdin.write(&cipher_opening);

    if args.execute {
        // --- 模式 A: Execute (模拟) ---
        println!("\nStarting SP1 execution (execute mode)...");
        let client = ProverClient::builder().light().build().await;
        let (output, report) = client.execute(VDD_WALRUS_RSLHVE_ELF, stdin).await.unwrap();

        // 验证输出
        if output.as_slice() == combined_expected.as_slice() {
            println!("✅ Output matches expected Triple-Binding commitments.");
        } else {
            println!("❌ Output MISMATCH!");
            println!("  Expected: {}", bytes_to_hex(&combined_expected, 96));
            println!("  Returned: {}", bytes_to_hex(output.as_slice(), 96));
        }

        println!("\n--- Performance Report ---");
        println!("  Cycles:             {}", report.total_instruction_count());
        println!(
            "  Unique Memory:      {} addresses",
            report.touched_memory_addresses
        );
        if let Some(gas) = report.gas() {
            println!("  Estimated Gas:      {}", gas);
        }
        println!("--------------------------");
    } else {
        // --- 模式 B: Prove (证明生成) ---
        println!("\nStarting SP1 setup and proving...");
        let client = ProverClient::builder().cpu().build().await;
        let pk = client.setup(VDD_WALRUS_RSLHVE_ELF).await.unwrap();

        let proof = client
            .prove(&pk, stdin.clone())
            .await
            .expect("failed to generate proof");
        println!("✅ Successfully generated ZK proof!");

        client
            .verify(&proof, &pk.verifying_key(), None)
            .expect("failed to verify proof");
        println!("✅ Successfully verified ZK proof!");

        // 获取公共输出
        let (output, _) = client.execute(VDD_WALRUS_RSLHVE_ELF, stdin).await.unwrap();
        println!("\nPublic Output Summary:");
        println!("  Hex: {}", bytes_to_hex(output.as_slice(), 96));
    }
}
