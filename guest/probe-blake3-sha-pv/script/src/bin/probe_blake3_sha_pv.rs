use clap::{Parser, ValueEnum};
use serde::Serialize;
use sp1_sdk::network::NetworkMode;
use sp1_sdk::{
    include_elf, Elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey,
    SP1ProofWithPublicValues, SP1Stdin, SP1VerifyingKey,
};
use std::path::PathBuf;

pub const PROBE_BLAKE3_SHA_PV_ELF: Elf = include_elf!("probe-blake3-sha-pv-program");

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

    let mut stdin = SP1Stdin::new();
    stdin.write(&args.n);

    let pk = client.setup(PROBE_BLAKE3_SHA_PV_ELF).await.unwrap();
    println!("probe-blake3-sha-pv sdk=6.2.4 public-values-hasher=sha256");
    println!("probe-blake3-sha-pv vkey={}", pk.verifying_key().bytes32());

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
    let fixture = Fixture {
        n,
        sp1_sdk_version: "6.2.4",
        public_values_hasher: "sha256",
        vkey: vk.bytes32().to_string(),
        public_values: format!("0x{}", hex::encode(proof.public_values.as_slice())),
        proof: format!("0x{}", hex::encode(proof.bytes())),
    };

    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    std::fs::create_dir_all(&fixture_dir).expect("failed to create fixture directory");
    std::fs::write(
        fixture_dir.join("probe-blake3-sha-pv-groth16-fixture.json"),
        serde_json::to_string_pretty(&fixture).unwrap(),
    )
    .expect("failed to write fixture");
    println!("probe-blake3-sha-pv fixture saved");
}
