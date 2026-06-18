#![no_main]
sp1_zkvm::entrypoint!(main);

pub fn main() {
    use sp1_zkvm::io::{commit, read};

    let length = read::<u8>();
    let msg = read::<Vec<u8>>();

    let mut digest_input = Vec::new();
    digest_input.push(length);
    digest_input.extend_from_slice(&(msg.len() as u32).to_le_bytes());
    digest_input.extend_from_slice(&msg);

    for _ in 0..length {
        let key = read::<[u8; 32]>();
        digest_input.extend_from_slice(&key);
    }

    for _ in 0..length {
        let nonce = read::<[u8; 12]>();
        digest_input.extend_from_slice(&nonce);
    }

    let digest: [u8; 32] = blake3::hash(&digest_input).into();
    commit(&length);
    commit(&(msg.len() as u32));
    commit(&digest);
}
