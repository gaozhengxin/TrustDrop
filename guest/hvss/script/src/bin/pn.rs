//! An end-to-end example of using the SP1 SDK to generate a proof of a program that can have an
//! EVM-Compatible proof generated which can be verified on-chain.
//!
//! You can run this script using the following command:
//! ```shell
//! RUST_LOG=info cargo run --release --bin pn -- --system groth16
//! ```
//! or
//! ```shell
//! RUST_LOG=info cargo run --release --bin pn -- --system plonk
//! ```
//! Generate an EVM-compatible proof for the ChaCha8 ZK program.

use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use sp1_sdk::{
    include_elf, HashableKey, ProverClient, Prover, SP1ProofWithPublicValues, SP1Stdin, SP1VerifyingKey
};
use sp1_sdk::network::NetworkMode;
use std::path::PathBuf;

use dotenv::dotenv;

use rand::{rngs::StdRng, SeedableRng};
use k256::ecdsa::SigningKey;
use k256::sha2::{Digest, Sha256};

use maenad_lib::data::MESSAGE_32;
use maenad_lib::kdf::key_derive;
use maenad_lib::common::{
    decode_public_outputs_with_cipher, print_public_outputs_with_cipher,
};
use maenad_lib::chacha8::derive_nonce;

use blake3;

/// ELF of your guest program
pub const HVSS_ELF: &[u8] = include_elf!("hvss-program");

/// CLI args
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct EVMArgs {
    #[arg(long, value_enum, default_value = "groth16")]
    system: ProofSystem,
}

/// Available proof systems
#[derive(Copy, Clone, PartialEq, Eq, ValueEnum, Debug)]
enum ProofSystem {
    Plonk,
    Groth16,
}

/// JSON fixture format for Solidity
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SP1ChaCha8ProofFixture {
    vkey: String,
    public_values: String,
    proof: String,
}

fn main() {
    dotenv().ok();
    sp1_sdk::utils::setup_logger();

    let args = EVMArgs::parse();
    println!("Selected Proof System: {:?}", args.system);

    //std::env::set_var("NETWORK_PRIVATE_KEY", "0x0000000000000000000000000000000000000000000000000000000000000000");
    let client = ProverClient::builder()
        .network_for(NetworkMode::Mainnet)
        .build();
    //std::env::set_var("NETWORK_PRIVATE_KEY", "");

    // -----------------------------------------
    // 1. Prepare Message & Derive Key
    // -----------------------------------------
    let msg: &[u8] = &MESSAGE_32;
    
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

    // -----------------------------------------
    // 2. Build zkVM input
    // -----------------------------------------
    let mut stdin = SP1Stdin::new();
    // 密钥长度输入
    stdin.write(&key_length);
    // 消息输入
    stdin.write_slice(msg);
    // 密钥输入
    stdin.write_slice(&key_1);
    stdin.write_slice(&key_2);
    stdin.write_slice(&key_3);
    stdin.write_slice(&key_4);
    // nonce 输入
    stdin.write_slice(nonce_1_ref);
    stdin.write_slice(nonce_2_ref);
    stdin.write_slice(nonce_3_ref);
    stdin.write_slice(nonce_4_ref);

    // -----------------------------------------
    // 3. Setup & Prove
    // -----------------------------------------
    let (pk, vk) = client.setup(HVSS_ELF);
    println!("Set up finished!");

    let proof: SP1ProofWithPublicValues = match args.system {
        ProofSystem::Plonk => client.prove(&pk, &stdin)
            .compressed()
            .plonk()
            .run(),
        ProofSystem::Groth16 => client.prove(&pk, &stdin)
            .compressed()
            .groth16()
            .run(),
    }
    .expect("failed to generate proof");

    println!("✔ Proof generated successfully!");

    // -----------------------------------------
    // 4. Decode public outputs (ChaCha8)
    // -----------------------------------------
    let pub_bytes = proof.public_values.as_slice();

    println!("Decoding public outputs...");
    match decode_public_outputs_with_cipher(pub_bytes) {
        Ok(decoded) => {
            println!("===== ChaCha8 Public Outputs =====");
            print_public_outputs_with_cipher(&decoded);
        }
        Err(e) => {
            eprintln!("❌ Failed to decode ZK outputs: {}", e);
        }
    }

    // -----------------------------------------
    // 5. Write fixture json (for Solidity)
    // -----------------------------------------
    write_fixture(&proof, &vk, args.system);
}

/// Save the proof fixture for Solidity.
fn write_fixture(proof: &SP1ProofWithPublicValues, vk: &SP1VerifyingKey, system: ProofSystem) {
    let public_values_hex = format!("0x{}", hex::encode(proof.public_values.as_slice()));
    let proof_hex = format!("0x{}", hex::encode(proof.bytes()));
    let vkey_hex = vk.bytes32().to_string();

    let fixture = SP1ChaCha8ProofFixture {
        vkey: vkey_hex,
        public_values: public_values_hex,
        proof: proof_hex,
    };

    let fixture_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../contracts/src/fixtures");

    std::fs::create_dir_all(&fixture_path).expect("failed to create fixture directory");
    let filename = format!("{:?}-fixture.json", system).to_lowercase();

    std::fs::write(
        fixture_path.join(filename),
        serde_json::to_string_pretty(&fixture).unwrap(),
    )
    .expect("failed to write fixture");

    println!("✔ Fixture saved for Solidity");
}
