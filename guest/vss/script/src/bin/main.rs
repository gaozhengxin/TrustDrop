//! An end-to-end example of using the SP1 SDK to generate a proof of a program that can be executed
//! or have a core proof generated.
//!
//! You can run this script using the following command:
//! ```shell
//! RUST_LOG=info cargo run --release -- --execute
//! ```
//! or
//! ```shell
//! RUST_LOG=info cargo run --release -- --prove
//! ```

use blake3;
use clap::Parser;
use drop_lib::chacha8::derive_nonce;
use drop_lib::common::{decode_public_outputs_with_cipher, print_public_outputs_with_cipher};
use drop_lib::data::MESSAGE_32;
use drop_lib::kdf::key_derive;
use k256::ecdsa::SigningKey;
use k256::sha2::{Digest, Sha256};
use rand::{rngs::StdRng, SeedableRng};
use sp1_sdk::Elf;
use sp1_sdk::Prover;
use sp1_sdk::ProvingKey;
use sp1_sdk::{include_elf, CpuProver, LightProver, ProverClient, SP1Stdin};
use std::time::Instant;

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const VSS_ELF: Elf = include_elf!("vss-program");

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    execute: bool,

    #[arg(long)]
    prove: bool,
}

#[tokio::main]
async fn main() {
    // Logger & env
    sp1_sdk::utils::setup_logger();
    dotenv::dotenv().ok();

    // Parse args
    let args = Args::parse();
    if args.execute == args.prove {
        eprintln!("Error: specify either --execute or --prove");
        std::process::exit(1);
    }

    // 准备数据
    //let msg: &[u8] = PLAINTEXT_DATA_1.as_bytes();
    let msg: &[u8] = &MESSAGE_32;
    println!("Original Message: {}", hex::encode(msg));

    let h_orig_block = blake3::hash(&msg);
    println!(
        "Original Message Hash (blake3): {}",
        hex::encode(h_orig_block.as_bytes())
    );

    let mut hasher = Sha256::new();
    hasher.update(msg);
    let msg_hash = hasher.finalize();
    let binding = msg_hash.try_into();
    let msg_hash_32: &[u8; 32] = match &binding {
        Ok(r) => r,
        Err(_) => {
            eprintln!("msg hash 数据长度不是 32 字节");
            return;
        }
    };

    println!("msg length = {:?}", msg.len());

    let key_length = 4u8; // 4 个密钥

    // 生成密钥对
    let mut rng_1 = StdRng::seed_from_u64(0x11111111);
    let sk_1 = SigningKey::random(&mut rng_1);
    let sk_1_bytes: [u8; 32] = sk_1.to_bytes().into();

    let mut rng_2 = StdRng::seed_from_u64(0x11112222);
    let sk_2 = SigningKey::random(&mut rng_2);
    let sk_2_bytes: [u8; 32] = sk_2.to_bytes().into();

    let mut rng_3 = StdRng::seed_from_u64(0x11113333);
    let sk_3 = SigningKey::random(&mut rng_3);
    let sk_3_bytes: [u8; 32] = sk_3.to_bytes().into();

    let mut rng_4 = StdRng::seed_from_u64(0x11114444);
    let sk_4 = SigningKey::random(&mut rng_4);
    let sk_4_bytes: [u8; 32] = sk_4.to_bytes().into();

    // 生成 chacha8 密钥
    let key_1 = match key_derive(&sk_1_bytes, &msg_hash_32) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("密钥推导失败: {}", e);
            return;
        }
    };

    let key_2 = match key_derive(&sk_2_bytes, &msg_hash_32) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("密钥推导失败: {}", e);
            return;
        }
    };

    let key_3 = match key_derive(&sk_3_bytes, &msg_hash_32) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("密钥推导失败: {}", e);
            return;
        }
    };

    let key_4 = match key_derive(&sk_4_bytes, &msg_hash_32) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("密钥推导失败: {}", e);
            return;
        }
    };

    // 计算 K_KEY 的承诺 (H_K)
    let h_k_1_calculated = blake3::hash(&key_1).as_bytes().to_vec();
    let h_k_2_calculated = blake3::hash(&key_2).as_bytes().to_vec();
    let h_k_3_calculated = blake3::hash(&key_3).as_bytes().to_vec();
    let h_k_4_calculated = blake3::hash(&key_4).as_bytes().to_vec();

    println!("Derived K_KEY 1: {}", hex::encode(key_1));
    println!("Derived K_KEY 2: {}", hex::encode(key_2));
    println!("Derived K_KEY 3: {}", hex::encode(key_3));
    println!("Derived K_KEY 4: {}", hex::encode(key_4));
    println!("H_K Commitment 1: {}", hex::encode(&h_k_1_calculated));
    println!("H_K Commitment 2: {}", hex::encode(&h_k_2_calculated));
    println!("H_K Commitment 3: {}", hex::encode(&h_k_3_calculated));
    println!("H_K Commitment 4: {}", hex::encode(&h_k_4_calculated));

    // 生成 chacha8 加密 nonce
    let binding = derive_nonce(&key_1, &msg_hash);
    let nonce_1_ref: &[u8; 12] = binding.as_slice().try_into().unwrap();
    let binding = derive_nonce(&key_2, &msg_hash);
    let nonce_2_ref: &[u8; 12] = binding.as_slice().try_into().unwrap();
    let binding = derive_nonce(&key_3, &msg_hash);
    let nonce_3_ref: &[u8; 12] = binding.as_slice().try_into().unwrap();
    let binding = derive_nonce(&key_4, &msg_hash);
    let nonce_4_ref: &[u8; 12] = binding.as_slice().try_into().unwrap();

    // Setup input (这里你可以修改为你的程序输入)
    let mut stdin = SP1Stdin::new();
    // 密钥长度输入
    stdin.write(&key_length);
    // 消息输入
    stdin.write(&msg);
    // 密钥输入
    stdin.write(&key_1);
    stdin.write(&key_2);
    stdin.write(&key_3);
    stdin.write(&key_4);
    // nonce 输入
    stdin.write(nonce_1_ref);
    stdin.write(nonce_2_ref);
    stdin.write(nonce_3_ref);
    stdin.write(nonce_4_ref);

    if args.execute {
        let client = ProverClient::builder().light().build().await;
        run_execute(&client, stdin, Vec::from([key_1, key_2, key_3, key_4])).await;
    } else {
        let client = ProverClient::builder().cpu().build().await;
        run_prove(&client, stdin, Vec::from([key_1, key_2, key_3, key_4])).await;
    }
}

fn handle_output_data(output_bytes: Vec<u8>, keys: Vec<[u8; 32]>) {
    println!("Raw output bytes: (Length: {})", output_bytes.len());
    println!("All bytes: {}", hex::encode(&output_bytes));

    match decode_public_outputs_with_cipher(&output_bytes) {
        Ok(decoded) => {
            // 调用打印函数
            print_public_outputs_with_cipher(&decoded);
            // 解密内容
            match decoded.decryptContent(keys) {
                Ok(decrypted_blocks) => {
                    for (i, block) in decrypted_blocks.iter().enumerate() {
                        println!("Decrypted Block[{}]: {}", i, hex::encode(block));
                    }
                }
                Err(e) => eprintln!("Error decrypt cipher: {}", e),
            }
        }
        Err(e) => {
            eprintln!("Error decoding ZK output: {}", e);
        }
    }
}

async fn run_execute(client: &LightProver, stdin: SP1Stdin, keys: Vec<[u8; 32]>) {
    let (output, report) = client.execute(VSS_ELF, stdin).await.unwrap();
    println!("Program executed successfully.");

    handle_output_data(output.as_slice().to_vec(), keys);

    println!(
        "Number of cycles executed: {}",
        report.total_instruction_count()
    );

    // 打印内存使用情况
    println!(
        "Unique Memory Touched:    {} addresses",
        report.touched_memory_addresses
    );

    // 打印 Gas 消耗（如果可用）
    if let Some(gas) = report.gas() {
        println!("Gas Used (Estimated):     {}", gas);
    } else {
        println!("Gas Used:                 (Not Calculated)");
    }

    println!("---------------------------------");
}

async fn run_prove(client: &CpuProver, stdin: SP1Stdin, keys: Vec<[u8; 32]>) {
    let pk = client.setup(VSS_ELF).await.unwrap();

    // --- 1. 证明生成计时 ---
    let start_prove = Instant::now();
    let proof: sp1_sdk::SP1ProofWithPublicValues = client.prove(&pk, stdin).await.unwrap();
    let duration_prove = start_prove.elapsed();

    println!("\n=========================================");
    println!("✅ Successfully generated proof!");
    println!("⏰ Proof Generation Time: {:.2?}", duration_prove);
    println!("=========================================");

    // --- 2. 证明验证计时 ---
    let start_verify = Instant::now();
    client
        .verify(&proof, &pk.verifying_key(), None)
        .expect("Failed to verify proof");
    let duration_verify = start_verify.elapsed();

    println!("\n✅ Successfully verified proof!");
    println!("⏰ Proof Verification Time: {:.2?}", duration_verify);
    println!("=========================================");

    // 3. 解码 Proof 的公开输出
    let output_vec = proof.public_values.as_slice().to_vec();
    handle_output_data(output_vec, keys);
}
