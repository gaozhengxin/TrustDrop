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

use clap::{ Parser, ValueEnum };
use serde::{ Deserialize, Serialize };
use sp1_sdk::{
    include_elf,
    HashableKey,
    ProverClient,
    Prover,
    SP1ProofWithPublicValues,
    SP1Stdin,
    SP1VerifyingKey,
};
use sp1_sdk::network::NetworkMode;
use std::path::PathBuf;

use dotenv::dotenv;

use rand::RngCore;
use maenad_lib::chacha8::chacha8_seal;
use maenad_lib::walrus_address::compute_blob_id_default;
use rand::rng;
use blake3;

/// ELF of your guest program
pub const VDD_WALRUS_ELF: &[u8] = include_elf!("program-vdd-walrus");

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
struct SP1VDDProofFixture {
    c_origin: String,
    c_key: String,
    c_cipher: String,
    data_length: u32,
    vkey: String,
    public_values: String,
    proof: String,
}

fn main() {
    dotenv().ok();
    sp1_sdk::utils::setup_logger();

    let args = EVMArgs::parse();
    println!("Selected Proof System: {:?}", args.system);

    let client = ProverClient::from_env();

    // === 1. Prepare inputs on host ===
    const ORIGIN_SIZE: u32 = 1 * 1024 * 1024;

    let mut origin = vec![0u8; ORIGIN_SIZE.try_into().unwrap()];
    rand::thread_rng().fill_bytes(&mut origin);

    // generate random 32-byte key
    let mut key_arr = [0u8; 32];
    rng().fill_bytes(&mut key_arr);
    let key_vec: Vec<u8> = key_arr.to_vec();

    // compute commitments
    let c_origin_raw: blake3::Hash = blake3::hash(&origin);
    let c_origin = c_origin_raw.as_bytes();

    let c_key_hash = blake3::hash(&key_vec);
    let c_key: Vec<u8> = c_key_hash.as_bytes().to_vec();

    // compute cipher
    let cipher = chacha8_seal(&origin, &key_arr, c_origin).expect("data encryption failed");

    // compute cipher commitment
    let cipher_blob_id = compute_blob_id_default(&cipher).expect(
        "Should compute blob ID for cipher data"
    );
    let c_cipher = cipher_blob_id.as_ref();

    // combined expected public output (what guest commit_slice will commit)
    let mut combined_expected: Vec<u8> = Vec::with_capacity(128);
    combined_expected.extend_from_slice(c_origin);
    combined_expected.extend_from_slice(&c_key);
    combined_expected.extend_from_slice(&c_cipher);
    combined_expected.extend_from_slice(&ORIGIN_SIZE.to_be_bytes());

    // print some info
    eprintln!("Prepared inputs:");
    eprintln!("  origin bytes: {}", origin.len());
    eprintln!("  key: 32 bytes");
    eprintln!("  c_origin (blake3): {}", bytes_to_hex_prefix(c_origin, 32));
    eprintln!("  c_key    (blake3): {}", bytes_to_hex_prefix(&c_key, 32));
    eprintln!("  c_cipher (blob id): {}", bytes_to_hex_prefix(&c_cipher, 32));

    // -----------------------------------------
    // 2. Build zkVM input
    // -----------------------------------------
    let mut stdin = SP1Stdin::new();

    stdin.write(&c_origin);
    stdin.write(&c_key);
    stdin.write(&c_cipher);
    stdin.write(&origin);
    stdin.write(&key_vec);

    // -----------------------------------------
    // 3. Setup & Prove
    // -----------------------------------------
    let (pk, vk) = client.setup(VDD_WALRUS_ELF);
    println!("Set up finished!");

    // idle
    if args.idle {
        write_fixture_args(
            format!("0x{}", hex::encode(c_origin_raw.as_bytes())),
            format!("0x{}", hex::encode(c_key_hash.as_bytes())),
            format!("0x{}", hex::encode(c_cipher)),
            cipher.len().try_into().unwrap(),
            &vk,
            args.system
        );
        return;
    }

    let proof: SP1ProofWithPublicValues = (
        match args.system {
            ProofSystem::Plonk => client.prove(&pk, &stdin).compressed().plonk().run(),
            ProofSystem::Groth16 => client.prove(&pk, &stdin).compressed().groth16().run(),
        }
    ).expect("failed to generate proof");

    println!("✔ Proof generated successfully!");

    // -----------------------------------------
    // 4. Write fixture json (for Solidity)
    // -----------------------------------------
    write_fixture(
        format!("0x{}", hex::encode(c_origin_raw.as_bytes())),
        format!("0x{}", hex::encode(c_key_hash.as_bytes())),
        format!("0x{}", hex::encode(c_cipher)),
        cipher.len().try_into().unwrap(),
        &proof,
        &vk,
        args.system
    );
}

/// Save the proof fixture for Solidity.
fn write_fixture(
    c_origin: String,
    c_key: String,
    c_cipher: String,
    data_length: u32,
    proof: &SP1ProofWithPublicValues,
    vk: &SP1VerifyingKey,
    system: ProofSystem
) {
    let public_values_hex = format!("0x{}", hex::encode(proof.public_values.as_slice()));
    let proof_hex = format!("0x{}", hex::encode(proof.bytes()));
    let vkey_hex = vk.bytes32().to_string();

    let fixture = SP1VDDProofFixture {
        c_origin: c_origin,
        c_key: c_key,
        c_cipher: c_cipher,
        data_length: data_length,
        vkey: vkey_hex,
        public_values: public_values_hex,
        proof: proof_hex,
    };

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../contracts/src/fixtures");

    std::fs::create_dir_all(&fixture_path).expect("failed to create fixture directory");
    let filename = format!("vdd-walrus-{:?}-fixture.json", system).to_lowercase();

    std::fs
        ::write(fixture_path.join(filename), serde_json::to_string_pretty(&fixture).unwrap())
        .expect("failed to write fixture");

    println!("✔ Fixture saved for Solidity");
}

fn write_fixture_args(
    c_origin: String,
    c_key: String,
    c_cipher: String,
    data_length: u32,
    vk: &SP1VerifyingKey,
    system: ProofSystem
) {
    let vkey_hex = vk.bytes32().to_string();

    let fixture = SP1VDDProofFixture {
        c_origin: c_origin,
        c_key: c_key,
        c_cipher: c_cipher,
        data_length: data_length,
        vkey: vkey_hex,
        public_values: "".to_string(),
        proof: "".to_string(),
    };

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../contracts/src/fixtures");

    std::fs::create_dir_all(&fixture_path).expect("failed to create fixture directory");
    let filename = format!("vdd-walrus-{:?}-fixture.json", system).to_lowercase();

    std::fs
        ::write(fixture_path.join(filename), serde_json::to_string_pretty(&fixture).unwrap())
        .expect("failed to write fixture");

    println!("✔ Fixture saved for Solidity");
}

fn bytes_to_hex_prefix(b: &[u8], prefix_len: usize) -> String {
    let mut out = String::new();
    let mut first = true;
    for &byte in b.iter().take(prefix_len) {
        if !first {
            out.push_str("");
        }
        first = false;
        out.push_str(&format!("{:02x}", byte));
    }
    out
}
