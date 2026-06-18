#![no_main]
sp1_zkvm::entrypoint!(main);

pub fn main() {
    use sp1_zkvm::io::{commit, read};

    let n = read::<u32>();
    let bytes = n.to_le_bytes();
    let digest: [u8; 32] = blake3::hash(&bytes).into();

    commit(&n);
    commit(&digest);
}
