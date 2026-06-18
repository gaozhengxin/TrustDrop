use clap::{Parser, ValueEnum};
use serde::Serialize;
use sp1_sdk::network::NetworkMode;
use sp1_sdk::{
    include_elf, Elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey,
    SP1ProofWithPublicValues, SP1Stdin, SP1VerifyingKey,
};
use std::path::PathBuf;

pub const PROBE_VSS_SHAPE_SHA_PV_ELF: Elf = include_elf!("probe-vss-shape-sha-pv-program");

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, value_enum, default_value = "groth16")]
    system: ProofSystem,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum ProofSystem {
    Groth16,
    Plonk,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    length: u8,
    message_len: u32,
    sp1_sdk_version: &'static str,
    public_values_hasher: &'static str,
    vkey: String,
    public_values: String,
    proof: String,
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    sp1_sdk::utils::setup_logger();

    let args = Args::parse();
    std::env::set_var("SP1_PROVER", "network");

    let client = ProverClient::builder()
        .network_for(NetworkMode::Mainnet)
        .build()
        .await;

    let length = 4u8;
    let msg: Vec<u8> = (0u8..32u8).collect();
    let keys = [[0x11u8; 32], [0x22u8; 32], [0x33u8; 32], [0x44u8; 32]];
    let nonces = [[0x55u8; 12], [0x66u8; 12], [0x77u8; 12], [0x88u8; 12]];

    let mut stdin = SP1Stdin::new();
    stdin.write(&length);
    stdin.write(&msg);
    for key in keys {
        stdin.write(&key);
    }
    for nonce in nonces {
        stdin.write(&nonce);
    }

    let pk = client.setup(PROBE_VSS_SHAPE_SHA_PV_ELF).await.unwrap();
    println!("probe-vss-shape-sha-pv sdk=6.2.4 public-values-hasher=sha256");
    println!(
        "probe-vss-shape-sha-pv vkey={}",
        pk.verifying_key().bytes32()
    );

    let proof = match args.system {
        ProofSystem::Groth16 => client
            .prove(&pk, stdin)
            .compressed()
            .groth16()
            .await
            .unwrap(),
        ProofSystem::Plonk => client.prove(&pk, stdin).compressed().plonk().await.unwrap(),
    };

    write_fixture(length, msg.len() as u32, &proof, &pk.verifying_key());
}

fn write_fixture(
    length: u8,
    message_len: u32,
    proof: &SP1ProofWithPublicValues,
    vk: &SP1VerifyingKey,
) {
    let fixture = Fixture {
        length,
        message_len,
        sp1_sdk_version: "6.2.4",
        public_values_hasher: "sha256",
        vkey: vk.bytes32().to_string(),
        public_values: format!("0x{}", hex::encode(proof.public_values.as_slice())),
        proof: format!("0x{}", hex::encode(proof.bytes())),
    };

    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    std::fs::create_dir_all(&fixture_dir).expect("failed to create fixture directory");
    std::fs::write(
        fixture_dir.join("probe-vss-shape-sha-pv-groth16-fixture.json"),
        serde_json::to_string_pretty(&fixture).unwrap(),
    )
    .expect("failed to write fixture");
    println!("probe-vss-shape-sha-pv fixture saved");
}
