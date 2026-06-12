#![no_std]

use light_poseidon::{Poseidon, PoseidonHasher, PoseidonBytesHasher, PoseidonError};
use ark_bn254::Fr;

pub fn poseidon_hash_fr(inputs: &[Fr]) -> Result<Fr, PoseidonError> {
    let params = light_poseidon::parameters::bn254_x5::get_poseidon_parameters::<Fr>(
        (2).try_into().map_err(|_| PoseidonError::U64Tou8)?,
    )?;

    let mut hasher = Poseidon::<Fr>::new(params);

    hasher.hash(inputs)
}

pub fn poseidon_hash_bytes(inputs: &[&[u8]]) -> Result<[u8; 32], PoseidonError> {
    let params = light_poseidon::parameters::bn254_x5::get_poseidon_parameters::<Fr>(
        (2).try_into().map_err(|_| PoseidonError::U64Tou8)?,
    )?;

    let fixed_inputs: Vec<Vec<u8>> = inputs
        .iter()
        .map(|slice| {
            if slice.len() == 32 {
                let mut v = slice.to_vec();
                // 小端序时最高有效字节在 index 31
                v[31] = 0;
                v
            } else {
                slice.to_vec()
            }
        })
        .collect();

    let fixed_refs: Vec<&[u8]> = fixed_inputs.iter().map(|v| v.as_slice()).collect();

    let mut hasher = Poseidon::<Fr>::new(params);
    hasher.hash_bytes_le(&fixed_refs)
}


pub fn poseidon_hash_single_bytes(input: &[u8]) -> Result<[u8; 32], PoseidonError> {
    let chunks: Vec<&[u8]> = input
        .chunks(32)
        .collect();

    poseidon_hash_bytes(&chunks)
}

/// Poseidon-based stream cipher:
///   stream = Poseidon(key || block_index)
///   ciphertext = plaintext XOR stream
///
/// NOTE: This is reversible and acts like a real encryption scheme.
/// Poseidon 仅用来生成伪随机流。
pub fn poseidon_encrypt(
    key: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, PoseidonError> {
    if key.is_empty() {
        return Err(PoseidonError::EmptyInput);
    }

    let mut output = Vec::with_capacity(plaintext.len());

    // 每 32 字节生成一个 Poseidon block
    let mut block_index: u64 = 0;

    for chunk in plaintext.chunks(32) {
        // 生成 PRG block: H(key || block_index)
        let mut key_material = key.to_vec();
        key_material.extend_from_slice(&block_index.to_be_bytes());

        let stream_block = poseidon_hash_single_bytes(&key_material).unwrap(); // 32 bytes

        // XOR
        for (i, &byte) in chunk.iter().enumerate() {
            output.push(byte ^ stream_block[i]);
        }

        block_index += 1;
    }

    Ok(output)
}

/// Decryption is identical since XOR is symmetric:
/// ciphertext XOR stream = plaintext
pub fn poseidon_decrypt(
    key: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, PoseidonError> {
    poseidon_encrypt(key, ciphertext)
}
