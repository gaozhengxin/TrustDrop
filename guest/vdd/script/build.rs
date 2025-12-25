use sp1_build::build_program_with_args;
use sp1_build::BuildArgs;

fn main() {
    // PROGRAM_VDD_WALRUS_ELF
    build_program_with_args("../program-vdd-walrus", BuildArgs {
        elf_name: Some("program-vdd-walrus".to_string()),
        ..Default::default()
    });

    // PROGRAM_VDD_FILECOIN_ELF
    build_program_with_args("../program-vdd-filecoin", BuildArgs {
        elf_name: Some("program-vdd-filecoin".to_string()),
        ..Default::default()
    });
}
