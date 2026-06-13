use sp1_sdk::{include_elf, Elf, HashableKey, Prover, ProverClient, ProvingKey};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const VSS_ELF: Elf = include_elf!("vss-program");

#[tokio::main]
async fn main() {
    let prover = ProverClient::builder().cpu().build().await;
    let pk = prover.setup(VSS_ELF).await.unwrap();
    println!("{}", pk.verifying_key().bytes32());
}
