use sp1_sdk::{include_elf, HashableKey, Prover, ProverClient};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const VDD_FILECOIN_ELF: &[u8] = include_elf!("program-vdd-filecoin");

fn main() {
    let prover = ProverClient::builder().cpu().build();
    let (_, vk) = prover.setup(VDD_FILECOIN_ELF);
    println!("{}", vk.bytes32());
}
