use sp1_build::{build_program_with_args, BuildArgs};

fn main() {
    build_program_with_args(
        "../program",
        BuildArgs {
            elf_name: Some("video-sampling-program".to_string()),
            ..Default::default()
        },
    );
}
