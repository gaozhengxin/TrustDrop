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
use sp1_sdk::network::FulfillmentStrategy;
use sp1_sdk::network::NetworkMode;
use sp1_sdk::Elf;
use sp1_sdk::ProveRequest;
use sp1_sdk::ProvingKey;
use sp1_sdk::{
    include_elf, HashableKey, Prover, ProverClient, SP1ProofWithPublicValues, SP1Stdin,
    SP1VerifyingKey,
};
use std::path::PathBuf;

use dotenv::dotenv;

use k256::ecdsa::SigningKey;
use k256::sha2::{Digest, Sha256};
use rand::{rngs::StdRng, SeedableRng};

use drop_lib::chacha8::derive_nonce;
use drop_lib::common::{decode_public_outputs_with_cipher, print_public_outputs_with_cipher};
use drop_lib::data::MESSAGE_32;
use drop_lib::kdf::key_derive;

use blake3;

/// ELF of your guest program
pub const VSS_ELF: Elf = include_elf!("vss-program");

/// CLI args
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct EVMArgs {
    #[arg(long, value_enum, default_value = "groth16")]
    system: ProofSystem,
    #[arg(long)]
    idle: bool,
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
struct SP1VSSProofFixture {
    length: u32,
    hOrigBlock: String,
    hKCommitment: Vec<String>,
    nonce: Vec<String>,
    vkey: String,
    public_values: String,
    proof: String,
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    sp1_sdk::utils::setup_logger();

    let args = EVMArgs::parse();
    println!("Selected Proof System: {:?}", args.system);

    std::env::set_var("SP1_PROVER", "network");
    let client = ProverClient::builder()
        .network_for(NetworkMode::Mainnet)
        //.private()
        .build()
        .await;
    //std::env::set_var("NETWORK_PRIVATE_KEY", "");

    // -----------------------------------------
    // 1. Prepare Message & Derive Key
    // -----------------------------------------
    let msg: &[u8] = &MESSAGE_32;

    let h_orig_block = blake3::hash(&msg);

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
    let h_k_1_calculated = blake3::hash(&key_1);
    let h_k_2_calculated = blake3::hash(&key_2);
    let h_k_3_calculated = blake3::hash(&key_3);
    let h_k_4_calculated = blake3::hash(&key_4);

    println!("Derived K_KEY 1: {}", hex::encode(key_1));
    println!("Derived K_KEY 2: {}", hex::encode(key_2));
    println!("Derived K_KEY 3: {}", hex::encode(key_3));
    println!("Derived K_KEY 4: {}", hex::encode(key_4));
    println!(
        "H_K Commitment 1: {}",
        hex::encode(&h_k_1_calculated.as_bytes().to_vec())
    );
    println!(
        "H_K Commitment 2: {}",
        hex::encode(&h_k_2_calculated.as_bytes().to_vec())
    );
    println!(
        "H_K Commitment 3: {}",
        hex::encode(&h_k_3_calculated.as_bytes().to_vec())
    );
    println!(
        "H_K Commitment 4: {}",
        hex::encode(&h_k_4_calculated.as_bytes().to_vec())
    );

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

    // -----------------------------------------
    // 3. Setup & Prove
    // -----------------------------------------
    let pk = client.setup(VSS_ELF).await.unwrap();

    // idle
    if args.idle {
        write_fixture_args(
            key_length as u32,
            format!("0x{}", hex::encode(h_orig_block.as_bytes())),
            vec![
                format!("0x{}", hex::encode(h_k_1_calculated.as_bytes())),
                format!("0x{}", hex::encode(h_k_2_calculated.as_bytes())),
                format!("0x{}", hex::encode(h_k_3_calculated.as_bytes())),
                format!("0x{}", hex::encode(h_k_4_calculated.as_bytes())),
            ],
            vec![
                format!("0x{}", hex::encode(nonce_1_ref)),
                format!("0x{}", hex::encode(nonce_2_ref)),
                format!("0x{}", hex::encode(nonce_3_ref)),
                format!("0x{}", hex::encode(nonce_4_ref)),
            ],
            &pk.verifying_key(),
            args.system,
        );
        return;
    }

    println!("🏃 正在本地执行 Guest 程序，捕捉虚拟机报错...");
    let (mut public_values, execution_report) =
        client.execute(VSS_ELF, stdin.clone()).await.unwrap();
    println!(
        "✔ Guest 程序模拟成功！消耗周期数: {}",
        execution_report.total_instruction_count()
    );

    let proof: SP1ProofWithPublicValues = (match args.system {
        ProofSystem::Plonk => client
            .prove(&pk, stdin)
            .compressed()
            //.strategy(FulfillmentStrategy::Auction)
            //.max_price_per_pgu(1_000_000u64)
            .plonk()
            .await
            .unwrap(),
        ProofSystem::Groth16 => client
            .prove(&pk, stdin)
            .compressed()
            //.strategy(FulfillmentStrategy::Auction)
            //.max_price_per_pgu(1_000_000u64)
            .groth16()
            .await
            .unwrap(),
    });

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
    write_fixture(
        key_length as u32,
        format!("0x{}", hex::encode(h_orig_block.as_bytes())),
        vec![
            format!("0x{}", hex::encode(h_k_1_calculated.as_bytes())),
            format!("0x{}", hex::encode(h_k_2_calculated.as_bytes())),
            format!("0x{}", hex::encode(h_k_3_calculated.as_bytes())),
            format!("0x{}", hex::encode(h_k_4_calculated.as_bytes())),
        ],
        vec![
            format!("0x{}", hex::encode(nonce_1_ref)),
            format!("0x{}", hex::encode(nonce_2_ref)),
            format!("0x{}", hex::encode(nonce_3_ref)),
            format!("0x{}", hex::encode(nonce_4_ref)),
        ],
        &proof,
        &pk.verifying_key(),
        args.system,
    );
}

/// Save the proof fixture for Solidity.
fn write_fixture(
    length: u32,
    h_orig_block: String,
    h_k_commitment: Vec<String>,
    nonce: Vec<String>,
    proof: &SP1ProofWithPublicValues,
    vk: &SP1VerifyingKey,
    system: ProofSystem,
) {
    let public_values_hex = format!("0x{}", hex::encode(proof.public_values.as_slice()));
    let proof_hex = format!("0x{}", hex::encode(proof.bytes()));
    let vkey_hex = vk.bytes32().to_string();

    let fixture = SP1VSSProofFixture {
        length: length,
        hOrigBlock: h_orig_block,
        hKCommitment: h_k_commitment,
        nonce: nonce,
        vkey: vkey_hex,
        public_values: public_values_hex,
        proof: proof_hex,
    };

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../contracts/src/fixtures");

    std::fs::create_dir_all(&fixture_path).expect("failed to create fixture directory");
    let filename = format!("{:?}-fixture.json", system).to_lowercase();

    std::fs::write(
        fixture_path.join(filename),
        serde_json::to_string_pretty(&fixture).unwrap(),
    )
    .expect("failed to write fixture");

    println!("✔ Fixture saved for Solidity");
}

fn write_fixture_args(
    length: u32,
    h_orig_block: String,
    h_k_commitment: Vec<String>,
    nonce: Vec<String>,
    vk: &SP1VerifyingKey,
    system: ProofSystem,
) {
    let vkey_hex = vk.bytes32().to_string();

    let fixture = SP1VSSProofFixture {
        length: length,
        hOrigBlock: h_orig_block,
        hKCommitment: h_k_commitment,
        nonce: nonce,
        vkey: vkey_hex,
        public_values: "".to_string(),
        proof: "".to_string(),
    };

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../contracts/src/fixtures");

    std::fs::create_dir_all(&fixture_path).expect("failed to create fixture directory");
    let filename = format!("{:?}-fixture.json", system).to_lowercase();

    std::fs::write(
        fixture_path.join(filename),
        serde_json::to_string_pretty(&fixture).unwrap(),
    )
    .expect("failed to write fixture");

    println!("✔ Fixture saved for Solidity");
}
