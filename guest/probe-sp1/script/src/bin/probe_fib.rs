use clap::{Parser, ValueEnum};
use serde::Serialize;
use sp1_sdk::network::NetworkMode;
use sp1_sdk::{
    include_elf, Elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey,
    SP1ProofWithPublicValues, SP1Stdin, SP1VerifyingKey,
};
use std::path::PathBuf;

pub const PROBE_FIB_ELF: Elf = include_elf!("probe-sp1-program");

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value_t = 20)]
    n: u32,
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
    n: u32,
    fibonacci: u32,
    sp1_sdk_version: &'static str,
    network_mode: &'static str,
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

    let mut stdin = SP1Stdin::new();
    stdin.write(&args.n);

    let pk = client.setup(PROBE_FIB_ELF).await.unwrap();
    println!("probe-sp1 sdk=6.2.4 network=mainnet");
    println!("probe-sp1 vkey={}", pk.verifying_key().bytes32());

    let proof = match args.system {
        ProofSystem::Groth16 => client
            .prove(&pk, stdin)
            .compressed()
            .groth16()
            .await
            .unwrap(),
        ProofSystem::Plonk => client.prove(&pk, stdin).compressed().plonk().await.unwrap(),
    };

    write_fixture(args.n, &proof, &pk.verifying_key());
}

fn write_fixture(n: u32, proof: &SP1ProofWithPublicValues, vk: &SP1VerifyingKey) {
    let public_values = proof.public_values.as_slice();
    assert_eq!(public_values.len(), 8, "unexpected public values length");

    let fib = u32::from_le_bytes(public_values[4..8].try_into().unwrap());
    let fixture = Fixture {
        n,
        fibonacci: fib,
        sp1_sdk_version: "6.2.4",
        network_mode: "mainnet",
        vkey: vk.bytes32().to_string(),
        public_values: format!("0x{}", hex::encode(public_values)),
        proof: format!("0x{}", hex::encode(proof.bytes())),
    };

    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    std::fs::create_dir_all(&fixture_dir).expect("failed to create fixture directory");
    std::fs::write(
        fixture_dir.join("probe-fib-groth16-fixture.json"),
        serde_json::to_string_pretty(&fixture).unwrap(),
    )
    .expect("failed to write fixture");
    println!("probe-sp1 fixture saved");
}
