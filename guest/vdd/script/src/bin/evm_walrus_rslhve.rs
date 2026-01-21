use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use sp1_sdk::{
    include_elf,
    HashableKey,
    ProverClient,
    SP1ProofWithPublicValues,
    SP1Stdin,
    SP1VerifyingKey,
};
use std::path::PathBuf;
use sha2::{Sha256, Digest};
use rand::{rng, RngCore};
use chacha20::cipher::{KeyIvInit, StreamCipher};
use chacha20::{ChaCha8, Key, Nonce};

// 引入你的 lib 逻辑
use maenad_lib::rslh_ve::{
    create_honest_proof, 
    derive_rslh_nonce,
    DEFAULT_SAMPLE_COUNT,
};
use maenad_lib::walrus_address::compute_blob_id_default;

/// ELF of the Walrus VDD program
pub const VDD_WALRUS_RSLHVE_ELF: &[u8] = include_elf!("program-vdd-walrus-rslhve");

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct EVMArgs {
    #[arg(long, value_enum, default_value = "groth16")]
    system: ProofSystem,
    #[arg(long)]
    idle: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum, Debug)]
enum ProofSystem {
    Plonk,
    Groth16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SP1VDDProofFixture {
    c_origin: String,
    c_key: String,
    c_cipher: String,
    vkey: String,
    public_values: String,
    proof: String,
}

fn main() {
    dotenv::dotenv().ok();
    sp1_sdk::utils::setup_logger();

    let args = EVMArgs::parse();
    let client = ProverClient::from_env();

    // 1. === Prepare inputs on host ===
    const DATA_SIZE: usize = 1 * 1024 * 1024; // 1MB
    let mut origin_data = vec![0u8; DATA_SIZE];
    rand::thread_rng().fill_bytes(&mut origin_data);

    let mut key = [0u8; 32];
    rng().fill_bytes(&mut key);

    // 计算三方绑定承诺
    let c_origin = compute_blob_id_default(&origin_data).unwrap();
    let c_origin_bytes: [u8; 32] = (*c_origin.as_ref()).try_into().unwrap();
    
    let c_key_hash = Sha256::digest(&key);
    let c_key_bytes: [u8; 32] = c_key_hash.into();

    let aux_data = b"maenad_v1";
    let nonce = derive_rslh_nonce(&key, aux_data);

    // 线性加密
    let mut cipher_data = origin_data.clone();
    let mut cipher_stream = ChaCha8::new(Key::from_slice(&key), Nonce::from_slice(&nonce));
    cipher_stream.apply_keystream(&mut cipher_data);
    
    let c_cipher = compute_blob_id_default(&cipher_data).unwrap();
    let c_cipher_bytes: [u8; 32] = (*c_cipher.as_ref()).try_into().unwrap();

    // 2. === Build SP1 stdin ===
    let mut stdin = SP1Stdin::new();
    stdin.write(&c_origin_bytes);
    stdin.write(&c_cipher_bytes);
    stdin.write(&c_key_bytes);
    stdin.write(&aux_data.to_vec());
    stdin.write(&key);

    // 生成采样证据
    let mut seed_h = Sha256::new();
    seed_h.update(&c_origin_bytes);
    seed_h.update(&c_cipher_bytes);
    seed_h.update(&c_key_bytes);
    let seed = seed_h.finalize();

    for i in 0..DEFAULT_SAMPLE_COUNT {
        let mut h = Sha256::new();
        h.update(&seed);
        h.update(&(i as u32).to_le_bytes());
        let idx = u32::from_le_bytes(h.finalize()[0..4].try_into().unwrap()) % 1000;

        let proof = create_honest_proof(&key, &nonce, idx, &origin_data, &cipher_data);
        stdin.write(&proof.global_index);
        stdin.write(&proof.origin_shard);
        stdin.write(&proof.cipher_shard);
    }

    // 3. === Setup & Prove ===
    let (pk, vk) = client.setup(VDD_WALRUS_RSLHVE_ELF);
    println!("SP1 Setup finished. Vkey: {}", vk.bytes32());

    if args.idle {
        println!("Idle mode: Skipping heavy proof generation...");
        write_fixture_data(
            &c_origin_bytes,
            &c_key_bytes,
            &c_cipher_bytes,
            None,
            &vk,
            args.system
        );
        return;
    }

    println!("Starting proof generation with {:?}...", args.system);
    let proof = match args.system {
        ProofSystem::Plonk => client.prove(&pk, &stdin).compressed().plonk().run(),
        ProofSystem::Groth16 => client.prove(&pk, &stdin).compressed().groth16().run(),
    }.expect("failed to generate proof");

    println!("✔ Proof generated successfully!");

    // 4. === Save Fixture ===
    write_fixture_data(
        &c_origin_bytes,
        &c_key_bytes,
        &c_cipher_bytes,
        Some(&proof),
        &vk,
        args.system
    );
}

fn write_fixture_data(
    c_origin: &[u8; 32],
    c_key: &[u8; 32],
    c_cipher: &[u8; 32],
    proof_data: Option<&SP1ProofWithPublicValues>,
    vk: &SP1VerifyingKey,
    system: ProofSystem
) {
    let (public_values_hex, proof_hex) = if let Some(p) = proof_data {
        (
            format!("0x{}", hex::encode(p.public_values.as_slice())),
            format!("0x{}", hex::encode(p.bytes()))
        )
    } else {
        ("0x".to_string(), "0x".to_string())
    };

    let fixture = SP1VDDProofFixture {
        c_origin: format!("0x{}", hex::encode(c_origin)),
        c_key: format!("0x{}", hex::encode(c_key)),
        c_cipher: format!("0x{}", hex::encode(c_cipher)),
        vkey: vk.bytes32().to_string(),
        public_values: public_values_hex,
        proof: proof_hex,
    };

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../contracts/src/fixtures");
    std::fs::create_dir_all(&fixture_path).expect("failed to create fixture directory");
    
    let filename = format!("walrus-{:?}-fixture.json", system).to_lowercase();
    std::fs::write(
        fixture_path.join(filename),
        serde_json::to_string_pretty(&fixture).unwrap()
    ).expect("failed to write fixture");

    println!("✔ Fixture saved to contracts/src/fixtures");
}