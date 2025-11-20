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
    sp1_sdk::utils::setup_logger();

    let args = EVMArgs::parse();
    println!("Selected Proof System: {:?}", args.system);

    std::env::set_var("NETWORK_PRIVATE_KEY", "0x0000000000000000000000000000000000000000000000000000000000000000");
    let client = ProverClient::builder()
        .network_for(NetworkMode::Mainnet)
        .build();
    std::env::set_var("NETWORK_PRIVATE_KEY", "");

    // -----------------------------------------
    // 1. Prepare Message & Derive Key
    // -----------------------------------------
    let msg: &[u8] = MESSAGE_32.as_slice();

    // sha256(msg)
    let mut hasher = Sha256::new();
    hasher.update(msg);
    let msg_hash = hasher.finalize();
    let msg_hash_32: &[u8; 32] = msg_hash.as_slice().try_into().unwrap();

    println!("msg length = {}", msg.len());

    // keypair -> derive chacha8 key
    let mut rng = StdRng::seed_from_u64(0x12345678);
    let sk = SigningKey::random(&mut rng);
    let sk_bytes: [u8; 32] = sk.to_bytes().into();

    let key = key_derive(&sk_bytes, msg_hash_32).expect("key derivation failed");
    let h_k = blake3::hash(&key).as_bytes().to_vec();

    println!("Derived K_KEY: {}", hex::encode(key));
    println!("H_K Commitment: {}", hex::encode(h_k));

    let nonce_vec = derive_nonce(&key, &msg_hash);
    let nonce_ref: &[u8; 12] = nonce_vec.as_slice().try_into().unwrap();

    // -----------------------------------------
    // 2. Build zkVM input
    // -----------------------------------------
    let mut stdin = SP1Stdin::new();
    stdin.write_slice(msg);
    stdin.write_slice(&key);
    stdin.write_slice(nonce_ref);

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
