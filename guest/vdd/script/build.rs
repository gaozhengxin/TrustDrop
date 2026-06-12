use sp1_build::build_program_with_args;
use sp1_build::BuildArgs;

fn main() {
    build_program_with_args(
        "../program-vdd-walrus-rslhve",
        BuildArgs {
            elf_name: Some("program-vdd-walrus-rslhve".to_string()),
            ..Default::default()
        },
    );
}
