//! A simple program that takes a number `n` as input, and writes the `n-1`th and `n`th fibonacci
//! number as an output.

// These two lines are necessary for the program to properly compile.
//
// Under the hood, we wrap your main function with some extra code so that it behaves properly
// inside the zkVM.
#![no_main]
sp1_zkvm::entrypoint!(main);

use maenad_lib::merkle;
use sp1_zkvm::io::commit;

pub fn main() {
    use sp1_zkvm::io::{read_vec, commit_slice};
    use blake3;

    // ================ public inputs ================
    // data commitment
    let c_origin_raw: Vec<u8> = read_vec();
    let c_origin = &c_origin_raw[8..];
    // chacha8 key commitment
    let c_key_raw = read_vec();
    let c_key = &c_key_raw[8..];
    // cipher commitment
    let c_cipher_raw: Vec<u8> = read_vec();
    let c_cipher = &c_cipher_raw[8..];
    // ================================================

    // ================ private inputs ================
    // origin data
    let origin_raw: Vec<u8> = read_vec();
    let origin = &origin_raw[8..];
    // chacha8 key
    let key_raw = read_vec();
    let key = &key_raw[8..];
    // ================================================

    const CHUNK_SIZE: usize = 1024 * 1024;
    let data_size: u32 = origin.len().try_into().unwrap();

    // ============ check data commitment =============
    let origin_mkt: merkle::MerkleTree = merkle::build_merkle_tree(origin, CHUNK_SIZE);
    let origin_mkt_root = origin_mkt.root();
    if c_origin != origin_mkt_root {
        panic!("origin data commitment mismatch");
    }
    // ================================================

    // ============= check key commitment =============
    let binding = blake3::hash(&key);
    let h_key = binding.as_bytes();
    if c_key != h_key {
        panic!("key commitment mismatch");
    }
    // ================================================

    // =========== check cipher commitment ============
    let key_arr_ref: &[u8; 32] = key.try_into().expect("key must be 32 bytes");
    let cipher_mkt: merkle::MerkleTree = merkle::encrypt_merkle_tree(&origin_mkt, key_arr_ref)
    .expect("data encryption failed");

    if c_cipher != cipher_mkt.root() {
        panic!("cipher commitment mismatch");
    }
    // ================================================

    // commitment
    let mut combined: Vec<u8> = Vec::with_capacity(128);
    combined.extend_from_slice(&c_origin);
    combined.extend_from_slice(&c_key);
    combined.extend_from_slice(&c_cipher);
    combined.extend_from_slice(&data_size.to_be_bytes());

    commit_slice(&combined);
}
