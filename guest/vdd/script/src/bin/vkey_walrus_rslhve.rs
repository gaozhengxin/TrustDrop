use sp1_sdk::{include_elf, Elf, HashableKey, Prover, ProverClient, ProvingKey};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const VDD_WALRUS_RSLHVE_ELF: Elf = include_elf!("program-vdd-walrus-rslhve");

#[tokio::main]
async fn main() {
    // VKey derivation only needs program setup; avoid loading the full CPU prover.
    let prover = ProverClient::builder().light().build().await;
    let pk = prover.setup(VDD_WALRUS_RSLHVE_ELF).await.unwrap();
    println!("{}", pk.verifying_key().bytes32());
}
