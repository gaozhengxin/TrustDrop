use sp1_sdk::{include_elf, Elf, HashableKey, Prover, ProverClient};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const VDD_WALRUS_RSLHVE_ELF: Elf = include_elf!("program-vdd-walrus-rslhve");

#[tokio::main]
async fn main() {
    let prover = ProverClient::builder().cpu().build().await;
    let pk = prover.setup(VDD_WALRUS_RSLHVE_ELF).await.unwrap();
    println!("{}", pk.verifying_key().bytes32());
}
