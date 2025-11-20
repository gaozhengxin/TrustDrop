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

use alloy_sol_types::SolType;
use clap::Parser;
use hvss_lib::PublicValuesStruct;
use sp1_sdk::{include_elf, ProverClient, SP1Stdin};
use sp1_sdk::env::EnvProver;
use rand::{rngs::StdRng, SeedableRng};
use k256::ecdsa::SigningKey;
use maenad_lib::data::PLAINTEXT_DATA_1;
use maenad_lib::kdf::key_derive;
use maenad_lib::common::{decode_public_outputs_with_cipher, print_public_outputs_with_cipher};
use maenad_lib::chacha8::derive_nonce;
use k256::sha2::{Digest, Sha256};
use blake3;
use std::time::Instant;

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const HVSS_ELF: &[u8] = include_elf!("hvss-program");

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    execute: bool,

    #[arg(long)]
    prove: bool,
}

fn main() {
    // Logger & env
    sp1_sdk::utils::setup_logger();
    dotenv::dotenv().ok();

    // Parse args
    let args = Args::parse();
    if args.execute == args.prove {
        eprintln!("Error: specify either --execute or --prove");
        std::process::exit(1);
    }

    // Setup client
    let client = ProverClient::from_env();

    // 准备数据
    let msg: &[u8] = PLAINTEXT_DATA_1.as_bytes();
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

    // 生成密钥对
    let mut rng = StdRng::seed_from_u64(0x12345678);
    let sk = SigningKey::random(&mut rng);
    let sk_bytes: [u8; 32] = sk.to_bytes().into();

    // 生成 chacha8 密钥
    let key_result = key_derive(&sk_bytes, &msg_hash_32);
    let key: [u8; 32] = match key_result {
        Ok(c) => c,
        Err(e) => {
            eprintln!("密钥推导失败: {}", e);
            return;
        }
    };

    // 计算 K_KEY 的承诺 (H_K)
    let h_k_calculated = blake3::hash(&key).as_bytes().to_vec();
    println!("Derived K_KEY: {}", hex::encode(key));
    println!("H_K Commitment: {}", hex::encode(&h_k_calculated));

    // 生成 chacha8 加密 nonce
    let binding = derive_nonce(&key, &msg_hash);
    let nonce_ref: &[u8; 12] = binding.as_slice().try_into().unwrap();

    // Setup input (这里你可以修改为你的程序输入)
    let mut stdin = SP1Stdin::new();
    // 消息输入
    stdin.write_slice(&msg);
    
    // 密钥输入
    stdin.write_slice(&key);

    // nonce 输入
    stdin.write_slice(nonce_ref);

    if args.execute {
        run_execute(&client, &stdin);
    } else {
        run_prove(&client, &stdin);
    }
}

fn handle_output_data(output_bytes: Vec<u8>) {
    println!("Raw output bytes: (Length: {})", output_bytes.len());

    match decode_public_outputs_with_cipher(&output_bytes) {
        Ok(decoded) => {
            // 调用打印函数
            print_public_outputs_with_cipher(&decoded);
        }
        Err(e) => {
            eprintln!("Error decoding ZK output: {}", e);
        }
    }
}

fn run_execute(client: &EnvProver, stdin: &SP1Stdin) {
    let (output, report) = client.execute(HVSS_ELF, stdin).run().unwrap();
    println!("Program executed successfully.");

    handle_output_data(output.as_slice().to_vec());

    println!("Number of cycles executed: {}", report.total_instruction_count());

    // 打印内存使用情况
    println!("Unique Memory Touched:    {} addresses", report.touched_memory_addresses);

    // 打印 Gas 消耗（如果可用）
    if let Some(gas) = report.gas {
        println!("Gas Used (Estimated):     {}", gas);
    } else {
        println!("Gas Used:                 (Not Calculated)");
    }

    println!("---------------------------------");
}

fn run_prove(client: &EnvProver, stdin: &SP1Stdin) {
    let (pk, vk) = client.setup(HVSS_ELF);
    
    // --- 1. 证明生成计时 ---
    let start_prove = Instant::now();
    let proof: sp1_sdk::SP1ProofWithPublicValues = client.prove(&pk, stdin).run().expect("Failed to generate proof");
    let duration_prove = start_prove.elapsed();
    
    println!("\n=========================================");
    println!("✅ Successfully generated proof!");
    println!("⏰ Proof Generation Time: {:.2?}", duration_prove);
    println!("=========================================");

    // --- 2. 证明验证计时 ---
    let start_verify = Instant::now();
    client.verify(&proof, &vk).expect("Failed to verify proof");
    let duration_verify = start_verify.elapsed();
    
    println!("\n✅ Successfully verified proof!");
    println!("⏰ Proof Verification Time: {:.2?}", duration_verify);
    println!("=========================================");
    
    // 3. 解码 Proof 的公开输出
    let output_vec = proof.public_values.as_slice().to_vec();
    handle_output_data(output_vec);
}
